use std::path::PathBuf;

use crate::types::{AppPaths, TableScribeError, TableScribeResult};

pub fn documents_dir() -> TableScribeResult<PathBuf> {
    dirs::document_dir()
        .ok_or_else(|| TableScribeError::Message("Could not resolve Documents folder".into()))
}

pub fn app_data_dir() -> TableScribeResult<PathBuf> {
    dirs::data_dir()
        .map(|path| path.join("TableScribe"))
        .ok_or_else(|| {
            TableScribeError::Message("Could not resolve application data folder".into())
        })
}

pub fn campaigns_dir() -> TableScribeResult<PathBuf> {
    Ok(documents_dir()?.join("TableScribe").join("Campaigns"))
}

pub fn models_dir() -> TableScribeResult<PathBuf> {
    Ok(app_data_dir()?.join("models"))
}

pub fn app_paths_for(platform: &str) -> TableScribeResult<AppPaths> {
    Ok(AppPaths {
        platform: platform.to_string(),
        documents_dir: documents_dir()?.display().to_string(),
        app_data_dir: app_data_dir()?.display().to_string(),
        campaigns_dir: campaigns_dir()?.display().to_string(),
        models_dir: models_dir()?.display().to_string(),
    })
}

pub fn normalize_filename(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            _ => character,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn app_paths() -> TableScribeResult<AppPaths> {
    app_paths_for("unsupported")
}

pub fn whisper_cli_sidecar_name() -> &'static str {
    "whisper-cli"
}

pub fn whisper_stream_sidecar_name() -> &'static str {
    "whisper-stream"
}
