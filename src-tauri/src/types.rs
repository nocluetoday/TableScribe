use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Campaign {
    pub id: String,
    pub name: String,
    pub system: Option<String>,
    pub description: Option<String>,
    pub created_at: String,
    pub root_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPaths {
    pub platform: String,
    pub documents_dir: String,
    pub app_data_dir: String,
    pub campaigns_dir: String,
    pub models_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub name: String,
    pub recommended_use: String,
    pub installed: bool,
    pub size_bytes: Option<u64>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionJob {
    pub id: String,
    pub campaign_id: String,
    pub session_name: String,
    pub audio_path: String,
    pub status: String,
    pub progress: u8,
    pub transcript_path: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    /// Stable identifier within the document. Today we generate `s0`, `s1`, …
    /// when first parsing whisper output; segment IDs persist across edits.
    pub id: String,
    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,
    /// Free-text speaker label. `null` means untagged. v1 is unstructured —
    /// renames are manual find-and-replace.
    pub speaker: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptDocument {
    pub schema_version: u32,
    pub session_name: String,
    pub created_at: String,
    pub model_path: Option<String>,
    pub segments: Vec<TranscriptSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub campaign_id: String,
    pub session_name: String,
    pub created_at: String,
    pub audio_path: String,
    pub transcript_md_path: Option<String>,
    pub transcript_txt_path: Option<String>,
    pub transcript_srt_path: Option<String>,
    pub notes_path: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum TableScribeError {
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("{0}")]
    Message(String),
}

pub type TableScribeResult<T> = Result<T, TableScribeError>;

impl serde::Serialize for TableScribeError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
