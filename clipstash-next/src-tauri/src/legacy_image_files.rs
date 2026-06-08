use crate::legacy_data::LegacyMessageImage;
use chrono::Local;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn remove_old_message_image_files(images: &[LegacyMessageImage]) {
    for image in images {
        let _ = fs::remove_file(&image.path);
    }
}

pub(crate) fn save_image_file(path: &Path, image_data: &[u8]) -> Result<(), String> {
    fs::write(path, image_data).map_err(|err| format!("保存图片文件失败：{err}"))
}

pub(crate) fn next_image_filename(images_dir: &Path, index: usize) -> String {
    let timestamp = Local::now().format("%Y%m%d%H%M%S%3f");
    let process_id = std::process::id();

    for attempt in 0.. {
        let suffix = if attempt == 0 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let filename = format!("clipstash-next-{timestamp}-{process_id}-{index}{suffix}.png");
        if !images_dir.join(&filename).exists() {
            return filename;
        }
    }

    unreachable!("image filename suffix search is unbounded");
}

pub(crate) fn resolve_legacy_image_path(
    data_dir: &Path,
    filename: &str,
) -> Result<PathBuf, String> {
    let trimmed = filename.trim();
    if trimmed.is_empty() {
        return Err("复制图片失败，图片文件名不能为空".to_string());
    }
    let candidate_name = Path::new(trimmed);
    if candidate_name.components().count() != 1 {
        return Err(format!("复制图片失败，非法图片文件名：{trimmed}"));
    }

    let image_path = data_dir.join("images").join(trimmed);
    if !image_path.is_file() {
        return Err(format!(
            "复制图片失败，图片文件不存在：{}",
            image_path.display()
        ));
    }

    Ok(image_path)
}
