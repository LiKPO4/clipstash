use crate::{
    app_data,
    legacy_image_files::sniff_image_extension,
    legacy_model::{LegacyMessage, MessageView, SortOrder},
    legacy_paths::path_to_string,
    legacy_query::list_legacy_messages_from_dir,
    legacy_schema::{configure_connection, ensure_legacy_schema},
};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

const EXPORT_SCHEMA_VERSION: u32 = 1;
const EXPORT_MANIFEST_NAME: &str = "clipstash-export.json";
const EXPORT_APP_VERSION: &str = env!("CARGO_PKG_VERSION");

// 导入防护上限：恶意或损坏的数据包可能在 manifest 里声明极小尺寸，实际解压出
// 巨量数据（zip 炸弹），若不设限会先耗尽内存再报错。以下上限均远高于正常使用
// 场景（单条消息图片极少超过数 MB、整包总量极少超过数 GB、消息数极少超过十万），
// 仅用于拒绝异常数据包，不影响正常导入。
const MAX_MANIFEST_BYTES: u64 = 64 * 1024 * 1024; // manifest 文本读取上限：64MB
const MAX_IMAGE_BYTES: u64 = 256 * 1024 * 1024; // 单张图片解压上限：256MB
const MAX_TOTAL_UNCOMPRESSED_BYTES: u64 = 4 * 1024 * 1024 * 1024; // 全包图片解压总量上限：4GB
const MAX_IMPORT_MESSAGES: usize = 100_000; // 消息数量上限：10 万条
const IMAGE_READ_CHUNK_SIZE: usize = 64 * 1024; // 图片分块读取的块大小：64KB

#[derive(Debug, Serialize)]
pub struct DataExportResult {
    pub path: String,
    pub message_count: i64,
    pub image_count: i64,
    pub skipped_archived_count: i64,
    pub skipped_missing_image_count: i64,
    pub skipped_empty_message_count: i64,
    pub bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct DataExportBytesResult {
    pub filename: String,
    pub export: DataExportResult,
    pub bytes: Vec<u8>,
    pub message_ids: Vec<i64>,
}

#[derive(Debug, Serialize)]
pub struct DataImportResult {
    pub path: String,
    pub inserted_messages: i64,
    pub skipped_messages: i64,
    pub imported_images: i64,
    pub stats: app_data::AppStats,
}

#[derive(Debug, Serialize)]
pub struct DataImportPreview {
    pub path: String,
    pub total_messages: i64,
    pub inserted_messages: i64,
    pub skipped_messages: i64,
    pub image_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExportManifest {
    schema_version: u32,
    app_version: String,
    exported_at: String,
    source_platform: String,
    messages: Vec<ExportMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExportMessage {
    text_content: Option<String>,
    created_at: String,
    images: Vec<ExportImage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExportImage {
    path: String,
    sha256: String,
    extension: String,
    size: u64,
}

pub fn export_normal_data_zip_to_path(output_path: PathBuf) -> Result<DataExportResult, String> {
    let stats = app_data::ensure_app_data_ready()?;
    let data_dir = app_data::app_data_dir_path()?;
    export_normal_data_zip_from_dir(&data_dir, output_path, stats.archived_count)
}

pub fn export_normal_data_zip_to_temp_bytes() -> Result<DataExportBytesResult, String> {
    let filename = default_export_filename();
    let temp_dir = std::env::temp_dir().join("ClipStash Next Exports");
    fs::create_dir_all(&temp_dir)
        .map_err(|err| format!("创建导出临时目录失败：{}：{err}", temp_dir.display()))?;
    // 清理上次导出遗留的临时 zip（保留本次即将生成的文件：Android 端
    // 分享流程仍依赖导出返回的 path 指向的文件存在，旧文件在下一次
    // 导出前不再被任何流程引用）
    remove_stale_export_temp_files(&temp_dir, &filename);
    let output_path = temp_dir.join(&filename);
    let stats = app_data::ensure_app_data_ready()?;
    let data_dir = app_data::app_data_dir_path()?;
    let (export, message_ids) =
        build_normal_data_zip_from_dir(&data_dir, output_path.clone(), stats.archived_count)?;
    let bytes = fs::read(&output_path)
        .map_err(|err| format!("读取导出数据包失败：{}：{err}", output_path.display()))?;

    Ok(DataExportBytesResult {
        filename,
        export,
        bytes,
        message_ids,
    })
}

fn export_normal_data_zip_from_dir(
    data_dir: &Path,
    output_path: PathBuf,
    skipped_archived_count: i64,
) -> Result<DataExportResult, String> {
    build_normal_data_zip_from_dir(data_dir, output_path, skipped_archived_count)
        .map(|(export, _)| export)
}

fn build_normal_data_zip_from_dir(
    data_dir: &Path,
    output_path: PathBuf,
    skipped_archived_count: i64,
) -> Result<(DataExportResult, Vec<i64>), String> {
    let output_path = ensure_zip_output_path(output_path)?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("创建导出目录失败：{}：{err}", parent.display()))?;
    }

    let mut messages = Vec::new();
    collect_messages(&data_dir, MessageView::Normal, &mut messages)?;

    let mut manifest_messages = Vec::new();
    let mut exported_message_ids = Vec::new();
    let mut staged_images = Vec::new();
    let mut skipped_missing_image_count = 0_i64;
    let mut skipped_empty_message_count = 0_i64;

    for (message_index, message) in messages.iter().enumerate() {
        let mut manifest_images = Vec::new();
        for (image_index, image) in message.images.iter().enumerate() {
            if !image.exists {
                skipped_missing_image_count += 1;
                continue;
            }

            let image_path = PathBuf::from(&image.path);
            let bytes = fs::read(&image_path)
                .map_err(|err| format!("读取导出图片失败：{}：{err}", image_path.display()))?;
            let sha256 = sha256_hex(&bytes);
            // 扩展名以图片内容魔数为准，避免把 JPEG 字节的图片以 .png 名写入数据包
            let extension = sniff_image_extension(&bytes).to_string();
            let zip_path = format!(
                "images/m{}-i{}-{}.{}",
                message_index + 1,
                image_index + 1,
                &sha256[..16],
                extension
            );

            manifest_images.push(ExportImage {
                path: zip_path.clone(),
                sha256,
                extension,
                size: bytes.len() as u64,
            });
            staged_images.push((zip_path, bytes));
        }

        let text_content = message
            .text_content
            .as_ref()
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty());
        if text_content.is_none() && manifest_images.is_empty() {
            skipped_empty_message_count += 1;
            continue;
        }

        manifest_messages.push(ExportMessage {
            text_content,
            created_at: message.created_at.clone(),
            images: manifest_images,
        });
        exported_message_ids.push(message.id);
    }

    let manifest = ExportManifest {
        schema_version: EXPORT_SCHEMA_VERSION,
        app_version: EXPORT_APP_VERSION.to_string(),
        exported_at: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        source_platform: std::env::consts::OS.to_string(),
        messages: manifest_messages,
    };
    write_export_zip(&output_path, &manifest, staged_images)?;

    let bytes = output_path
        .metadata()
        .map_err(|err| format!("读取导出文件信息失败：{}：{err}", output_path.display()))?
        .len();

    let export = DataExportResult {
        path: path_to_string(&output_path),
        message_count: manifest.messages.len() as i64,
        image_count: manifest
            .messages
            .iter()
            .map(|message| message.images.len() as i64)
            .sum(),
        skipped_archived_count,
        skipped_missing_image_count,
        skipped_empty_message_count,
        bytes,
    };
    Ok((export, exported_message_ids))
}

pub fn import_data_zip_from_path(zip_path: PathBuf) -> Result<DataImportResult, String> {
    let zip_path = validate_import_zip_path(zip_path)?;
    app_data::ensure_app_data_ready()?;
    let data_dir = app_data::app_data_dir_path()?;
    let (inserted_messages, skipped_messages, imported_images) =
        import_data_zip_into_dir(&zip_path, &data_dir)?;

    Ok(DataImportResult {
        path: path_to_string(&zip_path),
        inserted_messages,
        skipped_messages,
        imported_images,
        stats: app_data::read_app_stats()?,
    })
}

pub fn preview_data_zip_from_path(zip_path: PathBuf) -> Result<DataImportPreview, String> {
    let zip_path = validate_import_zip_path(zip_path)?;
    app_data::ensure_app_data_ready()?;
    let data_dir = app_data::app_data_dir_path()?;
    let (total_messages, inserted_messages, skipped_messages, image_count) =
        preview_data_zip_against_dir(&zip_path, &data_dir)?;

    Ok(DataImportPreview {
        path: path_to_string(&zip_path),
        total_messages,
        inserted_messages,
        skipped_messages,
        image_count,
    })
}

pub fn import_data_zip_from_bytes(
    filename: String,
    bytes: Vec<u8>,
) -> Result<DataImportResult, String> {
    validate_import_zip_filename(&filename)?;
    if bytes.is_empty() {
        return Err("导入数据包为空".to_string());
    }

    let temp_dir = std::env::temp_dir().join("ClipStash Next Imports");
    fs::create_dir_all(&temp_dir)
        .map_err(|err| format!("创建导入临时目录失败：{}：{err}", temp_dir.display()))?;
    let temp_path = temp_dir.join(format!(
        "clipstash-import-{}-{}.zip",
        Utc::now().timestamp_millis(),
        sanitize_zip_stem(&filename)
    ));
    fs::write(&temp_path, bytes)
        .map_err(|err| format!("写入导入临时数据包失败：{}：{err}", temp_path.display()))?;

    let result = import_data_zip_from_path(temp_path.clone());
    // 无论导入成功还是失败，都清理本次写入的临时数据包，
    // 避免 %TEMP%\ClipStash Next Imports 目录持续累积旧文件
    let _ = fs::remove_file(&temp_path);
    result
}

fn import_data_zip_into_dir(zip_path: &Path, data_dir: &Path) -> Result<(i64, i64, i64), String> {
    let db_path = data_dir.join("clipstash.db");
    let images_dir = data_dir.join("images");
    fs::create_dir_all(&images_dir).map_err(|err| format!("创建图片目录失败：{err}"))?;

    let mut archive = open_zip(zip_path)?;
    let manifest = read_manifest(&mut archive)?;
    validate_manifest(&manifest)?;

    let mut conn =
        Connection::open(&db_path).map_err(|err| format!("打开应用数据库准备导入失败：{err}"))?;
    configure_connection(&conn)?;
    ensure_legacy_schema(&conn)?;

    let mut saved_paths = Vec::new();
    let import_result = (|| {
        let tx = conn
            .transaction()
            .map_err(|err| format!("开启数据导入事务失败：{err}"))?;
        // 去重基准：只统计本次导入前库内已存在的消息签名，本次循环中
        // 新插入的消息不加入基准，因此包内重复消息也能全部导入；
        // 重复导入同一个包仍会命中首次导入已落库的签名，保持幂等
        let existing_signatures = load_existing_message_signatures(&tx, &images_dir)?;
        let mut total_uncompressed_bytes = 0_u64;
        let mut inserted_messages = 0_i64;
        let mut skipped_messages = 0_i64;
        let mut imported_images = 0_i64;

        for message in &manifest.messages {
            validate_import_message(message)?;
            let image_entries =
                read_message_images(&mut archive, message, &mut total_uncompressed_bytes)?;
            let image_hashes: Vec<String> = image_entries
                .iter()
                .map(|entry| entry.manifest.sha256.clone())
                .collect();

            if message_exists_by_signature(
                &existing_signatures,
                message.text_content.as_deref(),
                &message.created_at,
                &image_hashes,
            ) {
                skipped_messages += 1;
                continue;
            }

            tx.execute(
                "INSERT INTO messages (text_content, created_at, archived, archived_at)
                 VALUES (?, ?, 0, NULL)",
                params![message.text_content, message.created_at],
            )
            .map_err(|err| format!("导入消息失败：{err}"))?;
            let message_id = tx.last_insert_rowid();
            inserted_messages += 1;

            for (index, entry) in image_entries.into_iter().enumerate() {
                // 扩展名以图片内容魔数为准，修正旧数据包中 JPEG 字节使用 .png 扩展名的问题
                let filename = unique_imported_image_filename(
                    &images_dir,
                    message_id,
                    index + 1,
                    sniff_image_extension(&entry.bytes),
                    &entry.manifest.sha256,
                );
                let path = images_dir.join(&filename);
                fs::write(&path, &entry.bytes)
                    .map_err(|err| format!("写入导入图片失败：{}：{err}", path.display()))?;
                saved_paths.push(path.clone());
                tx.execute(
                    "INSERT INTO message_images (message_id, image_filename)
                     VALUES (?, ?)",
                    params![message_id, filename],
                )
                .map_err(|err| format!("写入导入图片关联失败：{err}"))?;
                imported_images += 1;
            }
        }

        tx.commit()
            .map_err(|err| format!("提交数据导入失败：{err}"))?;
        Ok::<(i64, i64, i64), String>((inserted_messages, skipped_messages, imported_images))
    })();

    let (inserted_messages, skipped_messages, imported_images) = match import_result {
        Ok(result) => result,
        Err(err) => {
            for path in saved_paths {
                let _ = fs::remove_file(path);
            }
            return Err(err);
        }
    };

    Ok((inserted_messages, skipped_messages, imported_images))
}

fn preview_data_zip_against_dir(
    zip_path: &Path,
    data_dir: &Path,
) -> Result<(i64, i64, i64, i64), String> {
    let db_path = data_dir.join("clipstash.db");
    let images_dir = data_dir.join("images");

    let mut archive = open_zip(zip_path)?;
    let manifest = read_manifest(&mut archive)?;
    validate_manifest(&manifest)?;

    let conn = Connection::open(&db_path)
        .map_err(|err| format!("打开应用数据库准备预览导入失败：{err}"))?;
    configure_connection(&conn)?;
    ensure_legacy_schema(&conn)?;

    // 与导入相同的去重基准语义，保证预览结果与真实导入一致
    let existing_signatures = load_existing_message_signatures(&conn, &images_dir)?;
    let mut total_uncompressed_bytes = 0_u64;
    let mut inserted_messages = 0_i64;
    let mut skipped_messages = 0_i64;
    let mut image_count = 0_i64;

    for message in &manifest.messages {
        validate_import_message(message)?;
        let image_entries =
            read_message_images(&mut archive, message, &mut total_uncompressed_bytes)?;
        image_count += image_entries.len() as i64;
        let image_hashes: Vec<String> = image_entries
            .iter()
            .map(|entry| entry.manifest.sha256.clone())
            .collect();

        if message_exists_by_signature(
            &existing_signatures,
            message.text_content.as_deref(),
            &message.created_at,
            &image_hashes,
        ) {
            skipped_messages += 1;
        } else {
            inserted_messages += 1;
        }
    }

    Ok((
        manifest.messages.len() as i64,
        inserted_messages,
        skipped_messages,
        image_count,
    ))
}

fn write_export_zip(
    output_path: &Path,
    manifest: &ExportManifest,
    staged_images: Vec<(String, Vec<u8>)>,
) -> Result<(), String> {
    let temp_path = output_path.with_extension("zip.tmp");
    let file = File::create(&temp_path)
        .map_err(|err| format!("创建导出 zip 失败：{}：{err}", temp_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let manifest_text =
        serde_json::to_vec_pretty(manifest).map_err(|err| format!("序列化导出清单失败：{err}"))?;

    zip.start_file(EXPORT_MANIFEST_NAME, options)
        .map_err(|err| format!("写入导出清单失败：{err}"))?;
    zip.write_all(&manifest_text)
        .map_err(|err| format!("写入导出清单失败：{err}"))?;

    for (zip_path, bytes) in staged_images {
        validate_zip_entry_path(&zip_path)?;
        zip.start_file(zip_path, options)
            .map_err(|err| format!("写入导出图片失败：{err}"))?;
        zip.write_all(&bytes)
            .map_err(|err| format!("写入导出图片失败：{err}"))?;
    }

    zip.finish()
        .map_err(|err| format!("完成导出 zip 失败：{err}"))?;
    if output_path.exists() {
        fs::remove_file(output_path)
            .map_err(|err| format!("替换旧导出文件失败：{}：{err}", output_path.display()))?;
    }
    fs::rename(&temp_path, output_path).map_err(|err| {
        format!(
            "保存导出 zip 失败：{} -> {}：{err}",
            temp_path.display(),
            output_path.display()
        )
    })
}

fn open_zip(path: &Path) -> Result<ZipArchive<File>, String> {
    let file =
        File::open(path).map_err(|err| format!("打开导入 zip 失败：{}：{err}", path.display()))?;
    ZipArchive::new(file).map_err(|err| format!("读取导入 zip 失败：{err}"))
}

fn read_manifest(archive: &mut ZipArchive<File>) -> Result<ExportManifest, String> {
    let mut manifest_file = archive
        .by_name(EXPORT_MANIFEST_NAME)
        .map_err(|_| format!("导入 zip 缺少 {EXPORT_MANIFEST_NAME}"))?;
    let mut text = String::new();
    manifest_file
        // 截断读取，防止声明极小实际巨量的 manifest 拖垮内存
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|err| format!("读取导入清单失败：{err}"))?;
    if text.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(format!("导入清单超过大小上限：{} 字节", MAX_MANIFEST_BYTES));
    }
    serde_json::from_str(&text).map_err(|err| format!("解析导入清单失败：{err}"))
}

fn read_message_images(
    archive: &mut ZipArchive<File>,
    message: &ExportMessage,
    total_uncompressed_bytes: &mut u64,
) -> Result<Vec<ImportImageEntry>, String> {
    let mut entries = Vec::new();
    for image in &message.images {
        validate_import_image(image)?;
        let mut image_file = archive
            .by_name(&image.path)
            .map_err(|_| format!("导入 zip 缺少图片：{}", image.path))?;
        // 先用 zip 条目声明的解压大小与 manifest 声明比对，不一致直接拒绝，
        // 避免解压出与清单不符的巨量数据
        if image_file.size() != image.size {
            return Err(format!("导入图片大小不匹配：{}", image.path));
        }
        if image.size > MAX_IMAGE_BYTES {
            return Err(format!(
                "导入图片超过单张大小上限：{}（{} 字节，上限 {} 字节）",
                image.path, image.size, MAX_IMAGE_BYTES
            ));
        }
        let mut bytes = Vec::with_capacity(image.size as usize);
        let mut chunk = [0_u8; IMAGE_READ_CHUNK_SIZE];
        loop {
            let read = image_file
                .read(&mut chunk)
                .map_err(|err| format!("读取导入图片失败：{}：{err}", image.path))?;
            if read == 0 {
                break;
            }
            *total_uncompressed_bytes += read as u64;
            if *total_uncompressed_bytes > MAX_TOTAL_UNCOMPRESSED_BYTES {
                return Err(format!(
                    "导入数据包图片解压总量超过上限：{} 字节",
                    MAX_TOTAL_UNCOMPRESSED_BYTES
                ));
            }
            bytes.extend_from_slice(&chunk[..read]);
        }
        // 与 zip 条目声明大小一致前提下的兜底校验
        if bytes.len() as u64 != image.size {
            return Err(format!("导入图片大小不匹配：{}", image.path));
        }
        let actual_hash = sha256_hex(&bytes);
        if actual_hash != image.sha256 {
            return Err(format!("导入图片校验失败：{}", image.path));
        }
        entries.push(ImportImageEntry {
            manifest: image.clone(),
            bytes,
        });
    }
    Ok(entries)
}

struct ImportImageEntry {
    manifest: ExportImage,
    bytes: Vec<u8>,
}

fn validate_manifest(manifest: &ExportManifest) -> Result<(), String> {
    if manifest.schema_version != EXPORT_SCHEMA_VERSION {
        return Err(format!("不支持的数据包版本：{}", manifest.schema_version));
    }
    if manifest.messages.len() > MAX_IMPORT_MESSAGES {
        return Err(format!(
            "导入清单消息数量超过上限：{} 条（上限 {} 条）",
            manifest.messages.len(),
            MAX_IMPORT_MESSAGES
        ));
    }
    Ok(())
}

fn validate_import_message(message: &ExportMessage) -> Result<(), String> {
    let has_text = message
        .text_content
        .as_ref()
        .map(|text| !text.trim().is_empty())
        .unwrap_or(false);
    if !has_text && message.images.is_empty() {
        return Err("导入清单包含空消息".to_string());
    }
    if message.created_at.trim().is_empty() {
        return Err("导入清单包含空创建时间".to_string());
    }
    Ok(())
}

fn validate_import_image(image: &ExportImage) -> Result<(), String> {
    validate_zip_entry_path(&image.path)?;
    if image.sha256.len() != 64 || !image.sha256.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(format!("导入图片校验值非法：{}", image.path));
    }
    if image.size == 0 {
        return Err(format!("导入图片为空：{}", image.path));
    }
    let expected_extension = safe_extension(&format!("image.{}", image.extension));
    if image.extension != expected_extension {
        return Err(format!("导入图片扩展名非法：{}", image.path));
    }
    Ok(())
}

fn validate_zip_entry_path(path: &str) -> Result<(), String> {
    if path.trim().is_empty()
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\\')
        || path.contains("//")
        || path.contains("../")
        || path.contains("/..")
        || !path.starts_with("images/")
        || path.ends_with('/')
    {
        return Err(format!("导入图片路径非法：{path}"));
    }
    Ok(())
}

fn validate_import_zip_path(path: PathBuf) -> Result<PathBuf, String> {
    if !path.is_file() {
        return Err(format!("导入数据包不存在：{}", path.display()));
    }
    if path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| !value.eq_ignore_ascii_case("zip"))
        .unwrap_or(true)
    {
        return Err("导入数据包必须是 .zip 文件".to_string());
    }
    Ok(path)
}

fn ensure_zip_output_path(path: PathBuf) -> Result<PathBuf, String> {
    if path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
    {
        Ok(path)
    } else {
        Ok(path.with_extension("zip"))
    }
}

fn default_export_filename() -> String {
    format!(
        "clipstash-export-{}.zip",
        Utc::now().format("%Y%m%d-%H%M%S")
    )
}

/// 清理导出临时目录中上一次遗留的 clipstash 导出 zip（含写入中断残留的
/// .zip.tmp），保留本次即将生成的文件：Android 端分享流程仍依赖导出
/// 返回的 path 指向的文件存在，因此只清理旧文件。
fn remove_stale_export_temp_files(temp_dir: &Path, keep_filename: &str) {
    let Ok(entries) = fs::read_dir(temp_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == keep_filename || !name.starts_with("clipstash-export-") {
            continue;
        }
        let _ = fs::remove_file(entry.path());
    }
}

fn validate_import_zip_filename(filename: &str) -> Result<(), String> {
    let name = filename.trim();
    if name.is_empty() {
        return Err("导入数据包文件名为空".to_string());
    }
    if !Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
    {
        return Err("导入数据包必须是 .zip 文件".to_string());
    }
    Ok(())
}

fn sanitize_zip_stem(filename: &str) -> String {
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("data");
    let safe = stem
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
        .take(40)
        .collect::<String>();
    if safe.is_empty() {
        "data".to_string()
    } else {
        safe
    }
}

fn collect_messages(
    data_dir: &Path,
    view: MessageView,
    messages: &mut Vec<LegacyMessage>,
) -> Result<(), String> {
    let mut offset = 0;
    loop {
        let page = list_legacy_messages_from_dir(
            data_dir.to_path_buf(),
            view,
            SortOrder::Oldest,
            Some(offset),
            Some(100),
        )?;
        offset += page.messages.len() as i64;
        messages.extend(page.messages);
        if !page.has_more {
            break;
        }
    }
    Ok(())
}

fn load_existing_message_signatures(
    conn: &Connection,
    images_dir: &Path,
) -> Result<HashMap<(Option<String>, String), Vec<String>>, String> {
    let mut stmt = conn
        .prepare("SELECT id, text_content, created_at FROM messages")
        .map_err(|err| format!("准备导入去重基准查询失败：{err}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|err| format!("查询导入去重基准失败：{err}"))?;
    let mut signatures = HashMap::new();
    for row in rows {
        let (message_id, text_content, created_at) =
            row.map_err(|err| format!("读取导入去重基准失败：{err}"))?;
        let hashes = read_message_image_hashes(conn, images_dir, message_id)?;
        signatures.insert((text_content, created_at), hashes);
    }
    Ok(signatures)
}

fn message_exists_by_signature(
    existing_signatures: &HashMap<(Option<String>, String), Vec<String>>,
    text_content: Option<&str>,
    created_at: &str,
    image_hashes: &[String],
) -> bool {
    let key = (
        text_content.map(|text| text.to_string()),
        created_at.to_string(),
    );
    existing_signatures
        .get(&key)
        .map(|hashes| hashes == image_hashes)
        .unwrap_or(false)
}

fn read_message_image_hashes(
    conn: &Connection,
    images_dir: &Path,
    message_id: i64,
) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT image_filename FROM message_images WHERE message_id = ? ORDER BY id")
        .map_err(|err| format!("准备导入图片去重查询失败：{err}"))?;
    let rows = stmt
        .query_map([message_id], |row| row.get::<_, String>(0))
        .map_err(|err| format!("查询导入图片去重失败：{err}"))?;

    let mut hashes = Vec::new();
    for row in rows {
        let filename = row.map_err(|err| format!("读取导入图片去重失败：{err}"))?;
        let path = images_dir.join(filename);
        if !path.is_file() {
            return Ok(Vec::new());
        }
        let bytes = fs::read(&path)
            .map_err(|err| format!("读取导入图片去重文件失败：{}：{err}", path.display()))?;
        hashes.push(sha256_hex(&bytes));
    }
    Ok(hashes)
}

fn unique_imported_image_filename(
    images_dir: &Path,
    message_id: i64,
    image_index: usize,
    extension: &str,
    sha256: &str,
) -> String {
    for attempt in 0.. {
        let suffix = if attempt == 0 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let filename = format!(
            "imported-{message_id}-{image_index}-{}{}.{extension}",
            &sha256[..16],
            suffix
        );
        if !images_dir.join(&filename).exists() {
            return filename;
        }
    }
    unreachable!("imported image filename suffix search is unbounded");
}

fn safe_extension(filename: &str) -> String {
    let extension = Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| value.chars().all(|ch| ch.is_ascii_alphanumeric()))
        .filter(|value| !value.is_empty() && value.len() <= 8)
        .unwrap_or_else(|| "png".to_string());
    match extension.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" => extension,
        _ => "png".to_string(),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut text = String::with_capacity(digest.len() * 2);
    for byte in digest {
        text.push_str(&format!("{byte:02x}"));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::legacy_query::read_legacy_stats_from_dir;
    use rusqlite::Connection;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn isolated_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("clipstash-transfer-{name}-{nonce}"))
    }

    fn seed_app_data(name: &str) -> PathBuf {
        let data_dir = isolated_dir(name);
        let images_dir = data_dir.join("images");
        fs::create_dir_all(&images_dir).unwrap();
        let conn = Connection::open(data_dir.join("clipstash.db")).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text_content TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                archived INTEGER DEFAULT 0,
                archived_at TIMESTAMP
            );
            CREATE TABLE message_images (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id INTEGER NOT NULL,
                image_filename TEXT NOT NULL
            );
            CREATE TABLE migration_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                migrated_at TEXT NOT NULL,
                legacy_db_path TEXT,
                legacy_images_dir TEXT,
                legacy_message_count INTEGER NOT NULL,
                legacy_image_count INTEGER NOT NULL
            );
            INSERT INTO migration_state VALUES (1, '2026-01-01 00:00:00', NULL, NULL, 0, 0);
            INSERT INTO messages (id, text_content, created_at, archived, archived_at) VALUES
                (1, 'normal text', '2026-01-01 08:00:00', 0, NULL),
                (2, 'archived text', '2026-01-02 08:00:00', 1, '2026-01-03 08:00:00'),
                (3, 'mixed text', '2026-01-04 08:00:00', 0, NULL);
            INSERT INTO message_images (message_id, image_filename) VALUES (3, 'one.png'), (3, 'two.png');
            ",
        )
        .unwrap();
        fs::write(images_dir.join("one.png"), b"image-one").unwrap();
        fs::write(images_dir.join("two.png"), b"image-two").unwrap();
        data_dir
    }

    #[test]
    fn exports_only_normal_messages() {
        let data_dir = seed_app_data("export-normal");
        let zip_path = data_dir.join("export.zip");

        let (result, message_ids) =
            build_normal_data_zip_from_dir(&data_dir, zip_path.clone(), 1).unwrap();

        assert_eq!(result.message_count, 2);
        assert_eq!(result.image_count, 2);
        assert_eq!(result.skipped_archived_count, 1);
        assert_eq!(message_ids, vec![1, 3]);

        let mut archive = open_zip(&zip_path).unwrap();
        let manifest = read_manifest(&mut archive).unwrap();
        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.messages.len(), 2);
        assert!(manifest
            .messages
            .iter()
            .all(|message| message.text_content.as_deref() != Some("archived text")));
        assert_eq!(manifest.messages[1].images.len(), 2);

        let _ = fs::remove_dir_all(data_dir);
    }

    #[test]
    fn imports_zip_and_skips_duplicate_second_import() {
        let export_dir = isolated_dir("roundtrip-export");
        fs::create_dir_all(&export_dir).unwrap();
        let zip_path = export_dir.join("export.zip");

        let source_data = seed_app_data("import-roundtrip");
        export_normal_data_zip_from_dir(&source_data, zip_path.clone(), 1).unwrap();

        let target_data = isolated_dir("import-target");
        fs::create_dir_all(target_data.join("images")).unwrap();
        let conn = Connection::open(target_data.join("clipstash.db")).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text_content TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                archived INTEGER DEFAULT 0,
                archived_at TIMESTAMP
            );
            CREATE TABLE message_images (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id INTEGER NOT NULL,
                image_filename TEXT NOT NULL
            );
            ",
        )
        .unwrap();

        let preview_before_import = preview_data_zip_against_dir(&zip_path, &target_data).unwrap();
        assert_eq!(preview_before_import, (2, 2, 0, 2));
        let stats_before_import = read_legacy_stats_from_dir(target_data.clone()).unwrap();
        assert_eq!(stats_before_import.total_count, 0);

        let first = import_data_zip_into_dir(&zip_path, &target_data).unwrap();
        assert_eq!(first, (2, 0, 2));

        let preview_after_import = preview_data_zip_against_dir(&zip_path, &target_data).unwrap();
        assert_eq!(preview_after_import, (2, 0, 2, 2));

        let second = import_data_zip_into_dir(&zip_path, &target_data).unwrap();
        assert_eq!(second, (0, 2, 0));

        let stats = read_legacy_stats_from_dir(target_data.clone()).unwrap();
        assert_eq!(stats.total_count, 2);
        assert_eq!(stats.archived_count, 0);

        let _ = fs::remove_dir_all(source_data);
        let _ = fs::remove_dir_all(target_data);
        let _ = fs::remove_dir_all(export_dir);
    }

    #[test]
    fn rejects_missing_manifest_without_changing_database() {
        let data_dir = seed_app_data("missing-manifest");
        let zip_path = data_dir.join("bad.zip");
        let file = File::create(&zip_path).unwrap();
        ZipWriter::new(file).finish().unwrap();
        let before = read_legacy_stats_from_dir(data_dir.clone()).unwrap();

        let result = import_data_zip_into_dir(&zip_path, &data_dir);

        assert!(result.unwrap_err().contains("缺少"));
        let after = read_legacy_stats_from_dir(data_dir.clone()).unwrap();
        assert_eq!(before.total_count, after.total_count);
        let _ = fs::remove_dir_all(data_dir);
    }

    #[test]
    fn rejects_illegal_image_entry_path() {
        let data_dir = seed_app_data("illegal-entry");
        let zip_path = data_dir.join("bad-entry.zip");
        let manifest = ExportManifest {
            schema_version: 1,
            app_version: "test".to_string(),
            exported_at: "2026-01-01 00:00:00".to_string(),
            source_platform: "test".to_string(),
            messages: vec![ExportMessage {
                text_content: Some("bad".to_string()),
                created_at: "2026-01-01 00:00:00".to_string(),
                images: vec![ExportImage {
                    path: "../bad.png".to_string(),
                    sha256: sha256_hex(b"bad"),
                    extension: "png".to_string(),
                    size: 3,
                }],
            }],
        };
        write_export_zip(&zip_path, &manifest, Vec::new()).unwrap();

        let result = import_data_zip_into_dir(&zip_path, &data_dir);

        assert!(result.unwrap_err().contains("路径非法"));
        let _ = fs::remove_dir_all(data_dir);
    }

    fn seed_empty_import_dir(name: &str) -> PathBuf {
        let data_dir = isolated_dir(name);
        fs::create_dir_all(data_dir.join("images")).unwrap();
        let conn = Connection::open(data_dir.join("clipstash.db")).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                text_content TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                archived INTEGER DEFAULT 0,
                archived_at TIMESTAMP
            );
            CREATE TABLE message_images (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id INTEGER NOT NULL,
                image_filename TEXT NOT NULL
            );
            ",
        )
        .unwrap();
        data_dir
    }

    fn duplicate_text_same_second_manifest() -> ExportManifest {
        ExportManifest {
            schema_version: 1,
            app_version: "test".to_string(),
            exported_at: "2026-01-01 00:00:00".to_string(),
            source_platform: "test".to_string(),
            messages: vec![
                ExportMessage {
                    text_content: Some("same text".to_string()),
                    created_at: "2026-01-01 00:00:00".to_string(),
                    images: Vec::new(),
                },
                ExportMessage {
                    text_content: Some("same text".to_string()),
                    created_at: "2026-01-01 00:00:00".to_string(),
                    images: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn rejects_zip_entry_size_mismatch_with_manifest() {
        let data_dir = seed_empty_import_dir("size-mismatch");
        let zip_path = data_dir.join("bad-size.zip");
        let manifest = ExportManifest {
            schema_version: 1,
            app_version: "test".to_string(),
            exported_at: "2026-01-01 00:00:00".to_string(),
            source_platform: "test".to_string(),
            messages: vec![ExportMessage {
                text_content: Some("size mismatch".to_string()),
                created_at: "2026-01-01 00:00:00".to_string(),
                images: vec![ExportImage {
                    path: "images/size-mismatch.png".to_string(),
                    sha256: sha256_hex(b"abc"),
                    extension: "png".to_string(),
                    size: 4, // manifest 谎报大小，zip 条目实际只有 3 字节
                }],
            }],
        };
        write_export_zip(
            &zip_path,
            &manifest,
            vec![("images/size-mismatch.png".to_string(), b"abc".to_vec())],
        )
        .unwrap();

        let result = import_data_zip_into_dir(&zip_path, &data_dir);

        assert!(result.unwrap_err().contains("大小不匹配"));
        let stats = read_legacy_stats_from_dir(data_dir.clone()).unwrap();
        assert_eq!(stats.total_count, 0);
        let _ = fs::remove_dir_all(data_dir);
    }

    #[test]
    fn imports_duplicate_text_same_second_messages_in_one_package() {
        let data_dir = seed_empty_import_dir("dup-in-package");
        let zip_path = data_dir.join("dup.zip");
        write_export_zip(
            &zip_path,
            &duplicate_text_same_second_manifest(),
            Vec::new(),
        )
        .unwrap();

        let result = import_data_zip_into_dir(&zip_path, &data_dir);

        assert_eq!(result.unwrap(), (2, 0, 0));
        let stats = read_legacy_stats_from_dir(data_dir.clone()).unwrap();
        assert_eq!(stats.total_count, 2);
        let _ = fs::remove_dir_all(data_dir);
    }

    #[test]
    fn reimporting_same_package_is_idempotent() {
        let data_dir = seed_empty_import_dir("reimport-idempotent");
        let zip_path = data_dir.join("reimport.zip");
        write_export_zip(
            &zip_path,
            &duplicate_text_same_second_manifest(),
            Vec::new(),
        )
        .unwrap();

        let first = import_data_zip_into_dir(&zip_path, &data_dir).unwrap();
        assert_eq!(first, (2, 0, 0));
        let second = import_data_zip_into_dir(&zip_path, &data_dir).unwrap();
        assert_eq!(second, (0, 2, 0));

        let stats = read_legacy_stats_from_dir(data_dir.clone()).unwrap();
        assert_eq!(stats.total_count, 2);
        let _ = fs::remove_dir_all(data_dir);
    }

    #[test]
    fn export_temp_cleanup_removes_old_files_but_keeps_current() {
        let dir = isolated_dir("export-cleanup");
        fs::create_dir_all(&dir).unwrap();
        let keep = "clipstash-export-20260101-000001.zip";
        fs::write(dir.join(keep), b"current").unwrap();
        fs::write(dir.join("clipstash-export-20250101-000001.zip"), b"old").unwrap();
        fs::write(
            dir.join("clipstash-export-20250101-000002.zip.tmp"),
            b"stale",
        )
        .unwrap();
        fs::write(dir.join("unrelated.txt"), b"keep").unwrap();

        remove_stale_export_temp_files(&dir, keep);

        assert!(dir.join(keep).is_file(), "本次导出的文件必须保留");
        assert!(!dir.join("clipstash-export-20250101-000001.zip").exists());
        assert!(!dir
            .join("clipstash-export-20250101-000002.zip.tmp")
            .exists());
        assert!(dir.join("unrelated.txt").is_file(), "无关文件不受影响");
        let _ = fs::remove_dir_all(dir);
    }
}
