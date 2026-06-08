use std::{
    env,
    path::{Path, PathBuf},
};

pub(crate) fn legacy_data_dir() -> Result<PathBuf, String> {
    if let Some(appdata) = env::var_os("APPDATA") {
        return Ok(PathBuf::from(appdata).join("ClipStash"));
    }

    if let Some(user_profile) = env::var_os("USERPROFILE") {
        return Ok(PathBuf::from(user_profile).join("ClipStash"));
    }

    Err("无法定位 APPDATA 或 USERPROFILE，不能确定旧数据目录".to_string())
}

pub(crate) fn path_to_string(path: impl AsRef<Path>) -> String {
    path.as_ref().display().to_string()
}
