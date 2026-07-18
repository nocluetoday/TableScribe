use crate::{platform, types::TableScribeResult};

#[tauri::command]
pub fn get_app_paths() -> TableScribeResult<crate::types::AppPaths> {
    platform::app_paths()
}
