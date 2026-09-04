use crate::{
    app_data,
    legacy_image_files::sniff_image_extension,
    legacy_model::{LegacyMessage, LegacyMessageImage},
    legacy_paths::path_to_string,
    legacy_schema::{configure_connection, ensure_legacy_schema},
};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
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
pub struct DataExportFileResult {
    pub filename: String,
    pub export: DataExportResult,
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
    let result =
        export_normal_data_zip_to_temp_file(std::env::temp_dir().join("ClipStash Next Exports"))?;
    let bytes =
        fs::read(&result.export.path).map_err(|err| format!("读取导出数据包失败：{err}"))?;
    Ok(DataExportBytesResult {
        filename: result.filename,
        export: result.export,
        bytes,
        message_ids: result.message_ids,
    })
}

pub fn export_normal_data_zip_to_temp_file(
    temp_dir: PathBuf,
) -> Result<DataExportFileResult, String> {
    let filename = default_export_filename();
    fs::create_dir_all(&temp_dir)
        .map_err(|err| format!("创建导出临时目录失败：{}：{err}", temp_dir.display()))?;
    // 接收应用可能延迟读取之前分享的 URI，只清理超过保留期的文件。
    remove_stale_export_temp_files(&temp_dir, &filename);
    let output_path = temp_dir.join(&filename);
    let stats = app_data::ensure_app_data_ready()?;
    let data_dir = app_data::app_data_dir_path()?;
    let (export, message_ids) =
        build_normal_data_zip_from_dir(&data_dir, output_path.clone(), stats.archived_count)?;
    Ok(DataExportFileResult {
        filename,
        export,
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

    let mut manifest_messages = Vec::new();
    let mut exported_message_ids = Vec::new();
    let mut staged_images = Vec::new();
    let mut skipped_missing_image_count = 0_i64;
    let mut skipped_empty_message_count = 0_i64;

    crate::transfer_progress::stage("export_hash", None);
    visit_normal_messages(data_dir, |message_index, message| {
        let mut manifest_images = Vec::new();
        for (image_index, image) in message.images.iter().enumerate() {
            if !image.exists {
                skipped_missing_image_count += 1;
                continue;
            }

            let image_path = PathBuf::from(&image.path);
            let mut source = File::open(&image_path)
                .map_err(|err| format!("读取导出图片失败：{}：{err}", image_path.display()))?;
            let (sha256, size, extension) = stream_image(&mut source, &mut std::io::sink())?;
            // 扩展名以图片内容魔数为准，避免把 JPEG 字节的图片以 .png 名写入数据包
            let zip_path = format!(
                "images/m{}-i{}-{}.{}",
                message_index + 1,
                image_index + 1,
                &sha256[..16],
                extension
            );

            let entry = ExportImage {
                path: zip_path.clone(),
                sha256,
                extension,
                size,
            };
            manifest_images.push(entry.clone());
            staged_images.push((entry, image_path));
        }

        let text_content = message
            .text_content
            .as_ref()
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty());
        if text_content.is_none() && manifest_images.is_empty() {
            skipped_empty_message_count += 1;
            return Ok(());
        }

        manifest_messages.push(ExportMessage {
            text_content,
            created_at: message.created_at.clone(),
            images: manifest_images,
        });
        exported_message_ids.push(message.id);
        Ok(())
    })?;

    let manifest = ExportManifest {
        schema_version: EXPORT_SCHEMA_VERSION,
        app_version: EXPORT_APP_VERSION.to_string(),
        exported_at: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        source_platform: std::env::consts::OS.to_string(),
        messages: manifest_messages,
    };
    crate::transfer_progress::stage(
        "export_write",
        Some(staged_images.iter().map(|(entry, _)| entry.size).sum()),
    );
    write_export_zip(&output_path, &manifest, |zip, options| {
        for (entry, path) in staged_images {
            validate_zip_entry_path(&entry.path)?;
            zip.start_file(&entry.path, options)
                .map_err(|err| format!("写入导出图片失败：{err}"))?;
            let mut source = File::open(&path)
                .map_err(|err| format!("读取导出图片失败：{}：{err}", path.display()))?;
            let (hash, size, _) = stream_image(&mut source, zip)?;
            if hash != entry.sha256 || size != entry.size {
                return Err(format!("导出期间图片已变化，请重试：{}", path.display()));
            }
        }
        Ok(())
    })?;

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
    let data_dir = app_data::ready_app_data_dir_path()?;
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
    let data_dir = app_data::ready_app_data_dir_path()?;
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
        let existing_signatures = load_existing_message_signatures(&tx, &images_dir, &manifest)?;
        crate::transfer_progress::stage("import", manifest_image_bytes(&manifest));
        let mut total_uncompressed_bytes = 0_u64;
        let mut inserted_messages = 0_i64;
        let mut skipped_messages = 0_i64;
        let mut imported_images = 0_i64;

        for message in &manifest.messages {
            validate_import_message(message)?;
            let image_hashes: Vec<String> = message
                .images
                .iter()
                .map(|entry| entry.sha256.clone())
                .collect();

            if message_exists_by_signature(
                &existing_signatures,
                message.text_content.as_deref(),
                &message.created_at,
                &image_hashes,
            ) {
                for image in &message.images {
                    read_import_image(
                        &mut archive,
                        image,
                        &mut total_uncompressed_bytes,
                        &mut std::io::sink(),
                    )?;
                }
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

            for (index, entry) in message.images.iter().enumerate() {
                validate_import_image(entry)?;
                let temp_name = unique_imported_image_filename(
                    &images_dir,
                    message_id,
                    index + 1,
                    "tmp",
                    &entry.sha256,
                );
                let temp_path = images_dir.join(temp_name);
                let mut target = File::options()
                    .write(true)
                    .create_new(true)
                    .open(&temp_path)
                    .map_err(|err| format!("创建导入临时图片失败：{err}"))?;
                saved_paths.push(temp_path.clone());
                let extension = read_import_image(
                    &mut archive,
                    entry,
                    &mut total_uncompressed_bytes,
                    &mut target,
                )?;
                target
                    .flush()
                    .map_err(|err| format!("写入导入图片失败：{err}"))?;
                drop(target);
                // 扩展名以图片内容魔数为准，修正旧数据包中 JPEG 字节使用 .png 扩展名的问题
                let filename = unique_imported_image_filename(
                    &images_dir,
                    message_id,
                    index + 1,
                    &extension,
                    &entry.sha256,
                );
                let path = images_dir.join(&filename);
                fs::rename(&temp_path, &path)
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

        crate::transfer_progress::stage("commit", None);
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
    let existing_signatures = load_existing_message_signatures(&conn, &images_dir, &manifest)?;
    crate::transfer_progress::stage("preview", manifest_image_bytes(&manifest));
    let mut total_uncompressed_bytes = 0_u64;
    let mut inserted_messages = 0_i64;
    let mut skipped_messages = 0_i64;
    let mut image_count = 0_i64;

    for message in &manifest.messages {
        validate_import_message(message)?;
        for image in &message.images {
            read_import_image(
                &mut archive,
                image,
                &mut total_uncompressed_bytes,
                &mut std::io::sink(),
            )?;
        }
        image_count += message.images.len() as i64;
        let image_hashes: Vec<String> = message
            .images
            .iter()
            .map(|entry| entry.sha256.clone())
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

fn stream_image(
    reader: &mut impl Read,
    writer: &mut impl Write,
) -> Result<(String, u64, String), String> {
    let mut buffer = [0_u8; IMAGE_READ_CHUNK_SIZE];
    let mut header = Vec::with_capacity(12);
    let mut hash = Sha256::new();
    let mut size = 0_u64;
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|err| format!("读取导出图片失败：{err}"))?;
        if count == 0 {
            break;
        }
        header.extend_from_slice(&buffer[..count.min(12 - header.len())]);
        hash.update(&buffer[..count]);
        writer
            .write_all(&buffer[..count])
            .map_err(|err| format!("写入导出图片失败：{err}"))?;
        size += count as u64;
        crate::transfer_progress::advance(count as u64);
    }
    Ok((
        format!("{:x}", hash.finalize()),
        size,
        sniff_image_extension(&header).to_string(),
    ))
}

fn write_export_zip(
    output_path: &Path,
    manifest: &ExportManifest,
    write_images: impl FnOnce(&mut ZipWriter<File>, SimpleFileOptions) -> Result<(), String>,
) -> Result<(), String> {
    let temp_path = output_path.with_extension("zip.tmp");
    let file = File::options()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .map_err(|err| format!("创建导出 zip 失败：{}：{err}", temp_path.display()))?;
    let result = (|| {
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        zip.start_file(EXPORT_MANIFEST_NAME, options)
            .map_err(|err| format!("写入导出清单失败：{err}"))?;
        serde_json::to_writer_pretty(&mut zip, manifest)
            .map_err(|err| format!("写入导出清单失败：{err}"))?;

        write_images(&mut zip, options)?;

        crate::transfer_progress::stage("commit", None);
        let completed = zip
            .finish()
            .map_err(|err| format!("完成导出 zip 失败：{err}"))?;
        completed
            .sync_all()
            .map_err(|err| format!("保存导出 zip 内容失败：{err}"))?;
        drop(completed);
        // rename replaces an existing file on both Windows and Unix. Never unlink it first.
        fs::rename(&temp_path, output_path).map_err(|err| {
            format!(
                "保存导出 zip 失败：{} -> {}：{err}",
                temp_path.display(),
                output_path.display()
            )
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

#[cfg(test)]
fn write_fixture_zip(
    output_path: &Path,
    manifest: &ExportManifest,
    images: Vec<(String, Vec<u8>)>,
) -> Result<(), String> {
    write_export_zip(output_path, manifest, |zip, options| {
        for (name, bytes) in images {
            validate_zip_entry_path(&name)?;
            zip.start_file(name, options)
                .map_err(|err| err.to_string())?;
            zip.write_all(&bytes).map_err(|err| err.to_string())?;
        }
        Ok(())
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

fn read_import_image(
    archive: &mut ZipArchive<File>,
    image: &ExportImage,
    total_uncompressed_bytes: &mut u64,
    writer: &mut impl Write,
) -> Result<String, String> {
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
    let mut hash = Sha256::new();
    let mut size = 0_u64;
    let mut header = Vec::with_capacity(12);
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
        size += read as u64;
        if size > image.size || size > MAX_IMAGE_BYTES {
            return Err(format!("导入图片大小不匹配：{}", image.path));
        }
        header.extend_from_slice(&chunk[..read.min(12 - header.len())]);
        hash.update(&chunk[..read]);
        crate::transfer_progress::advance(read as u64);
        writer
            .write_all(&chunk[..read])
            .map_err(|err| format!("写入导入图片失败：{err}"))?;
    }
    // 与 zip 条目声明大小一致前提下的兜底校验
    if size != image.size {
        return Err(format!("导入图片大小不匹配：{}", image.path));
    }
    let actual_hash = format!("{:x}", hash.finalize());
    if actual_hash != image.sha256 {
        return Err(format!("导入图片校验失败：{}", image.path));
    }
    Ok(sniff_image_extension(&header).to_string())
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

fn manifest_image_bytes(manifest: &ExportManifest) -> Option<u64> {
    manifest
        .messages
        .iter()
        .flat_map(|message| &message.images)
        .try_fold(0_u64, |total, image| total.checked_add(image.size))
        .filter(|total| *total > 0)
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
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!(
        "clipstash-export-{}-{}-{}.zip",
        Utc::now().format("%Y%m%d-%H%M%S%3f"),
        std::process::id(),
        SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

/// 接收应用可能稍后才读取 URI，至少保留 24 小时；只清理应用自己的过期导出。
fn remove_stale_export_temp_files(temp_dir: &Path, keep_filename: &str) {
    let Ok(entries) = fs::read_dir(temp_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == keep_filename || !name.starts_with("clipstash-export-") {
            continue;
        }
        if !(name.ends_with(".zip") || name.ends_with(".zip.tmp")) {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            continue;
        }
        if !metadata
            .modified()
            .ok()
            .and_then(|time| time.elapsed().ok())
            .is_some_and(|age| age.as_secs() >= 24 * 60 * 60)
        {
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

fn visit_normal_messages(
    data_dir: &Path,
    mut visit: impl FnMut(usize, LegacyMessage) -> Result<(), String>,
) -> Result<(), String> {
    let conn = Connection::open_with_flags(
        data_dir.join("clipstash.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(|err| format!("打开导出数据库失败：{err}"))?;
    configure_connection(&conn)?;
    ensure_legacy_schema(&conn)?;
    // One statement retains a single SQLite snapshot and never repeats COUNT/OFFSET.
    let mut stmt = conn
        .prepare(
            "SELECT m.id,m.text_content,m.created_at,m.archived_at,i.id,i.image_filename
         FROM messages m LEFT JOIN message_images i ON i.message_id=m.id
         WHERE COALESCE(m.archived,0)=0 ORDER BY m.created_at,m.id,i.id",
        )
        .map_err(|err| format!("准备导出查询失败：{err}"))?;
    let mut rows = stmt
        .query([])
        .map_err(|err| format!("查询导出消息失败：{err}"))?;
    let mut current: Option<LegacyMessage> = None;
    let mut index = 0;
    let images_dir = data_dir.join("images");
    while let Some(row) = rows
        .next()
        .map_err(|err| format!("读取导出消息失败：{err}"))?
    {
        let id: i64 = row.get(0).map_err(|err| err.to_string())?;
        if current.as_ref().is_some_and(|message| message.id != id) {
            visit(index, current.take().unwrap())?;
            index += 1;
        }
        if current.is_none() {
            current = Some(LegacyMessage {
                id,
                text_content: row.get(1).map_err(|err| err.to_string())?,
                created_at: row.get(2).map_err(|err| err.to_string())?,
                archived: false,
                archived_at: row.get(3).map_err(|err| err.to_string())?,
                images: Vec::new(),
            });
        }
        if let Some(image_id) = row
            .get::<_, Option<i64>>(4)
            .map_err(|err| err.to_string())?
        {
            let filename: String = row.get(5).map_err(|err| err.to_string())?;
            let path = images_dir.join(&filename);
            current.as_mut().unwrap().images.push(LegacyMessageImage {
                id: image_id,
                filename,
                exists: path.is_file(),
                path: path_to_string(path),
            });
        }
    }
    if let Some(message) = current {
        visit(index, message)?;
    }
    Ok(())
}

type MessageSignatures = HashMap<(Option<String>, String), HashSet<Vec<String>>>;

fn load_existing_message_signatures(
    conn: &Connection,
    images_dir: &Path,
    manifest: &ExportManifest,
) -> Result<MessageSignatures, String> {
    crate::transfer_progress::stage("dedupe", None);
    load_candidate_signatures(conn, images_dir, manifest, |path| {
        if !path.is_file() {
            return Ok(None);
        }
        let mut file = File::open(path)
            .map_err(|err| format!("读取导入图片去重文件失败：{}：{err}", path.display()))?;
        stream_image(&mut file, &mut std::io::sink()).map(|(hash, _, _)| Some(hash))
    })
}

fn load_candidate_signatures(
    conn: &Connection,
    images_dir: &Path,
    manifest: &ExportManifest,
    mut hash_file: impl FnMut(&Path) -> Result<Option<String>, String>,
) -> Result<MessageSignatures, String> {
    let candidates: HashSet<_> = manifest
        .messages
        .iter()
        .map(|message| (message.text_content.clone(), message.created_at.clone()))
        .collect();
    if candidates.is_empty() {
        return Ok(HashMap::new());
    }
    let mut stmt = conn
        .prepare(
            "SELECT m.id, m.text_content, m.created_at, i.image_filename FROM messages m
                  LEFT JOIN message_images i ON i.message_id=m.id ORDER BY m.id, i.id",
        )
        .map_err(|err| format!("准备导入去重基准查询失败：{err}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|err| format!("查询导入去重基准失败：{err}"))?;
    let mut signatures: MessageSignatures = HashMap::new();
    let mut file_hashes = HashMap::<String, Option<String>>::new();
    let mut current: Option<(i64, (Option<String>, String), Vec<String>, bool)> = None;
    let finish = |current: &mut Option<(i64, (Option<String>, String), Vec<String>, bool)>,
                  signatures: &mut MessageSignatures| {
        if let Some((_, key, hashes, valid)) = current.take() {
            if valid {
                signatures.entry(key).or_default().insert(hashes);
            }
        }
    };
    for row in rows {
        let (message_id, text_content, created_at, filename) =
            row.map_err(|err| format!("读取导入去重基准失败：{err}"))?;
        if current.as_ref().is_some_and(|entry| entry.0 != message_id) {
            finish(&mut current, &mut signatures);
        }
        let key = (text_content, created_at);
        if !candidates.contains(&key) {
            continue;
        }
        let (_, _, hashes, valid) =
            current.get_or_insert_with(|| (message_id, key, Vec::new(), true));
        if let Some(filename) = filename {
            if !file_hashes.contains_key(&filename) {
                file_hashes.insert(filename.clone(), hash_file(&images_dir.join(&filename))?);
            }
            match &file_hashes[&filename] {
                Some(hash) => hashes.push(hash.clone()),
                None => *valid = false,
            }
        }
    }
    finish(&mut current, &mut signatures);
    Ok(signatures)
}

fn message_exists_by_signature(
    existing_signatures: &MessageSignatures,
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
        .map(|variants| variants.contains(image_hashes))
        .unwrap_or(false)
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
    #[cfg(target_os = "windows")]
    #[test]
    fn export_replace_failure_preserves_locked_target_and_retry_succeeds() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = seed_empty_import_dir("replace-locked-export");
        let output = dir.join("existing.zip");
        std::fs::write(&output, b"previous export").unwrap();
        // Allow readers but deny deletion/rename by a second handle.
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&output)
            .unwrap();
        let manifest = duplicate_text_same_second_manifest();
        assert!(super::write_fixture_zip(&output, &manifest, Vec::new())
            .unwrap_err()
            .contains("保存导出 zip 失败"));
        assert_eq!(std::fs::read(&output).unwrap(), b"previous export");
        assert!(!output.with_extension("zip.tmp").exists());
        drop(lock);
        super::write_fixture_zip(&output, &manifest, Vec::new()).unwrap();
        let mut archive = super::open_zip(&output).unwrap();
        assert_eq!(
            super::read_manifest(&mut archive).unwrap().messages.len(),
            2
        );
        drop(archive);
        assert!(!output.with_extension("zip.tmp").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn streamed_import_bounds_writes_and_rolls_back_late_corruption() {
        struct CountWriter {
            total: usize,
            largest: usize,
        }
        impl std::io::Write for CountWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.total += bytes.len();
                self.largest = self.largest.max(bytes.len());
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let dir = seed_empty_import_dir("stream-import-rollback");
        let zip_path = dir.join("stream.zip");
        let image_bytes = vec![5_u8; 2 * 1024 * 1024 + 19];
        let entries: Vec<super::ExportImage> = ["images/one.png", "images/two.png"]
            .into_iter()
            .map(|path| super::ExportImage {
                path: path.into(),
                sha256: super::sha256_hex(&image_bytes),
                extension: "png".into(),
                size: image_bytes.len() as u64,
            })
            .collect();
        let mut manifest = duplicate_text_same_second_manifest();
        manifest.messages.truncate(1);
        manifest.messages[0].images = entries.clone();
        super::write_fixture_zip(
            &zip_path,
            &manifest,
            entries
                .iter()
                .map(|entry| (entry.path.clone(), image_bytes.clone()))
                .collect(),
        )
        .unwrap();
        let mut archive = super::open_zip(&zip_path).unwrap();
        let mut writer = CountWriter {
            total: 0,
            largest: 0,
        };
        super::read_import_image(&mut archive, &entries[0], &mut 0, &mut writer).unwrap();
        assert_eq!(writer.total, image_bytes.len());
        assert!(writer.largest <= super::IMAGE_READ_CHUNK_SIZE);
        let mut total = super::MAX_TOTAL_UNCOMPRESSED_BYTES - 1;
        assert!(super::read_import_image(
            &mut archive,
            &entries[0],
            &mut total,
            &mut std::io::sink()
        )
        .unwrap_err()
        .contains("总量超过上限"));
        drop(archive);
        let corrupt_zip = dir.join("corrupt.zip");
        let mut corrupt = image_bytes.clone();
        corrupt[0] ^= 1;
        super::write_fixture_zip(
            &corrupt_zip,
            &manifest,
            vec![
                (entries[0].path.clone(), image_bytes.clone()),
                (entries[1].path.clone(), corrupt),
            ],
        )
        .unwrap();
        assert!(super::import_data_zip_into_dir(&corrupt_zip, &dir)
            .unwrap_err()
            .contains("校验失败"));
        let conn = rusqlite::Connection::open(dir.join("clipstash.db")).unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM messages", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(std::fs::read_dir(dir.join("images")).unwrap().count(), 0);
        assert_eq!(
            super::import_data_zip_into_dir(&zip_path, &dir).unwrap(),
            (1, 0, 2)
        );
        // Matching manifest signatures still must validate corrupt bytes on the skip path.
        assert!(super::import_data_zip_into_dir(&corrupt_zip, &dir)
            .unwrap_err()
            .contains("校验失败"));
        assert!(super::preview_data_zip_against_dir(&corrupt_zip, &dir)
            .unwrap_err()
            .contains("校验失败"));
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM messages", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(std::fs::read_dir(dir.join("images")).unwrap().count(), 2);
        drop(conn);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn export_walk_preserves_order_and_snapshot_during_writes() {
        for count in [100, 1000, 10000] {
            let dir = seed_empty_import_dir(&format!("export-walk-{count}"));
            let conn = rusqlite::Connection::open(dir.join("clipstash.db")).unwrap();
            conn.execute_batch(&format!("PRAGMA journal_mode=WAL;
                CREATE INDEX IF NOT EXISTS idx_walk_images ON message_images(message_id);
                WITH RECURSIVE seq(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM seq WHERE x<{count})
                INSERT INTO messages(id,text_content,created_at,archived)
                SELECT x,'text',printf('2024-01-%02d',1+x%28),CASE WHEN x%3=0 THEN 1 WHEN x%5=0 THEN NULL ELSE 0 END FROM seq;
                INSERT INTO message_images(id,message_id,image_filename) VALUES (2,1,'missing.png'),(1,1,'present.png');")).unwrap();
            std::fs::write(dir.join("images/present.png"), b"fixture").unwrap();
            let expected: Vec<i64> = conn.prepare("SELECT id FROM messages WHERE archived=0 OR archived IS NULL ORDER BY created_at,id").unwrap()
                .query_map([],|row| row.get(0)).unwrap().map(Result::unwrap).collect();
            let mut actual = Vec::new();
            super::visit_normal_messages(&dir, |index,message| {
                assert_eq!(index,actual.len());
                if index==0 {
                    conn.execute_batch("UPDATE messages SET archived=1 WHERE id>5;
                        INSERT INTO messages(text_content,created_at,archived) VALUES ('late','2024-03-01',0);").unwrap();
                }
                assert!(!message.archived);
                if message.id==1 {
                    assert_eq!(message.images.iter().map(|image|image.id).collect::<Vec<_>>(),vec![1,2]);
                    assert!(message.images[0].exists);
                    assert!(!message.images[1].exists);
                }
                actual.push(message.id);
                Ok(())
            }).unwrap();
            assert_eq!(actual, expected);
            let result = super::visit_normal_messages(&dir, |_, _| Err("stop on failure".into()));
            assert_eq!(result.unwrap_err(), "stop on failure");
            drop(conn);
            std::fs::remove_dir_all(dir).unwrap();
        }
    }

    #[test]
    fn candidate_signatures_keep_variants_and_hash_only_relevant_files_once() {
        let dir = seed_empty_import_dir("signature-candidates");
        let conn = rusqlite::Connection::open(dir.join("clipstash.db")).unwrap();
        conn.execute_batch(
            "INSERT INTO messages(id,text_content,created_at) VALUES
            (1,'same','time'),(2,'same','time'),(3,'same','time'),
            (4,'unrelated','time'),(5,'broken','time');
            INSERT INTO message_images(message_id,image_filename) VALUES
            (1,'a.png'),(2,'b.png'),(3,'a.png'),(3,'b.png'),
            (4,'must-not-read.png'),(5,'missing.png');",
        )
        .unwrap();
        let mut manifest = duplicate_text_same_second_manifest();
        manifest.messages = ["same", "broken"]
            .into_iter()
            .map(|text| super::ExportMessage {
                text_content: Some(text.into()),
                created_at: "time".into(),
                images: Vec::new(),
            })
            .collect();
        let mut reads = std::collections::HashMap::<String, usize>::new();
        let signatures =
            super::load_candidate_signatures(&conn, &dir.join("images"), &manifest, |path| {
                let name = path.file_name().unwrap().to_str().unwrap().to_string();
                assert_ne!(name, "must-not-read.png");
                *reads.entry(name.clone()).or_default() += 1;
                Ok(if name == "missing.png" {
                    None
                } else {
                    Some(name)
                })
            })
            .unwrap();
        assert_eq!(reads.len(), 3);
        assert!(reads.values().all(|count| *count == 1));
        for hashes in [
            vec!["a.png".to_string()],
            vec!["b.png".to_string()],
            vec!["a.png".to_string(), "b.png".to_string()],
        ] {
            assert!(super::message_exists_by_signature(
                &signatures,
                Some("same"),
                "time",
                &hashes
            ));
        }
        assert!(!super::message_exists_by_signature(
            &signatures,
            Some("same"),
            "time",
            &["b.png".into(), "a.png".into()]
        ));
        assert!(!super::message_exists_by_signature(
            &signatures,
            Some("broken"),
            "time",
            &[]
        ));
        drop(conn);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn image_stream_is_bounded_and_hashes_short_reads_correctly() {
        struct Chunks {
            remaining: usize,
            max_requested: usize,
        }
        impl std::io::Read for Chunks {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                self.max_requested = self.max_requested.max(buffer.len());
                let count = self.remaining.min(buffer.len()).min(317);
                buffer[..count].fill(7);
                self.remaining -= count;
                Ok(count)
            }
        }
        let size = 8 * 1024 * 1024 + 19;
        let mut reader = Chunks {
            remaining: size,
            max_requested: 0,
        };
        let (hash, actual, extension) =
            super::stream_image(&mut reader, &mut std::io::sink()).unwrap();
        assert_eq!(actual, size as u64);
        assert_eq!(hash, super::sha256_hex(&vec![7; size]));
        assert_eq!(extension, "png");
        assert_eq!(reader.max_requested, super::IMAGE_READ_CHUNK_SIZE);
        let jpeg = b"\xff\xd8\xffpayload";
        let mut copied = Vec::new();
        let (_, _, extension) = super::stream_image(&mut &jpeg[..], &mut copied).unwrap();
        assert_eq!(extension, "jpg");
        assert_eq!(copied, jpeg);
    }

    #[test]
    fn failed_export_cleans_own_temp_and_preserves_previous_output() {
        let data_dir = seed_empty_import_dir("failed-stream-export");
        let output = data_dir.join("existing.zip");
        std::fs::write(&output, b"previous export").unwrap();
        let result = super::write_export_zip(
            &output,
            &duplicate_text_same_second_manifest(),
            |zip, options| {
                zip.start_file("images/partial.png", options).unwrap();
                std::io::Write::write_all(zip, b"partial").unwrap();
                Err("injected read failure".into())
            },
        );
        assert!(result.unwrap_err().contains("injected read failure"));
        assert_eq!(std::fs::read(&output).unwrap(), b"previous export");
        assert!(!output.with_extension("zip.tmp").exists());
        std::fs::write(output.with_extension("zip.tmp"), b"other operation").unwrap();
        assert!(super::write_export_zip(
            &output,
            &duplicate_text_same_second_manifest(),
            |_, _| Ok(())
        )
        .is_err());
        assert_eq!(
            std::fs::read(output.with_extension("zip.tmp")).unwrap(),
            b"other operation"
        );
        std::fs::remove_dir_all(data_dir).unwrap();
    }

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
        write_fixture_zip(&zip_path, &manifest, Vec::new()).unwrap();

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
        write_fixture_zip(
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
        write_fixture_zip(
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
        write_fixture_zip(
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
        fs::write(dir.join("clipstash-export-recent.zip"), b"recent").unwrap();
        let old = SystemTime::now() - std::time::Duration::from_secs(48 * 60 * 60);
        for name in [
            "clipstash-export-20250101-000001.zip",
            "clipstash-export-20250101-000002.zip.tmp",
        ] {
            File::options()
                .write(true)
                .open(dir.join(name))
                .unwrap()
                .set_modified(old)
                .unwrap();
        }

        remove_stale_export_temp_files(&dir, keep);

        assert!(dir.join(keep).is_file(), "本次导出的文件必须保留");
        assert!(!dir.join("clipstash-export-20250101-000001.zip").exists());
        assert!(!dir
            .join("clipstash-export-20250101-000002.zip.tmp")
            .exists());
        assert!(dir.join("unrelated.txt").is_file(), "无关文件不受影响");
        assert!(
            dir.join("clipstash-export-recent.zip").is_file(),
            "最近分享的文件应可继续读取"
        );
        let _ = fs::remove_dir_all(dir);
    }
}
