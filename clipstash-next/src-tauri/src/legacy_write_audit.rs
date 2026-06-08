use crate::{
    legacy_backup::{LegacyDbBackup, LegacyImageFilesBackup},
    legacy_data::LegacyMessage,
};
use serde::Serialize;

#[derive(Serialize)]
pub struct LegacyWriteAudit {
    pub operation: String,
    pub message_id: i64,
    pub db_backup_path: String,
    pub image_backup_dir: Option<String>,
}

pub(crate) fn legacy_write_audit(
    operation: &str,
    message: &LegacyMessage,
    backup: &LegacyDbBackup,
    image_backup: Option<&LegacyImageFilesBackup>,
) -> LegacyWriteAudit {
    LegacyWriteAudit {
        operation: operation.to_string(),
        message_id: message.id,
        db_backup_path: backup.backup_path.clone(),
        image_backup_dir: image_backup.map(|backup| backup.backup_dir.clone()),
    }
}
