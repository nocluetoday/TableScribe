use std::fs;

use crate::{commands::campaigns::find_campaign, platform, types::TableScribeResult};

#[tauri::command]
pub fn export_session(
    campaign_id: String,
    session_name: String,
    format: String,
) -> TableScribeResult<String> {
    let campaign = find_campaign(&campaign_id)?;
    let root = std::path::PathBuf::from(campaign.root_path);
    let safe_name = platform::normalize_filename(&session_name);
    let source_path = match format.as_str() {
        "md" => root.join("transcripts").join(format!("{}.md", safe_name)),
        "txt" => root.join("transcripts").join(format!("{}.txt", safe_name)),
        "srt" => root.join("transcripts").join(format!("{}.srt", safe_name)),
        "json" => root.join("campaign.json"),
        _ => root.join("transcripts").join(format!("{}.md", safe_name)),
    };

    let mut export_path = root.join("exports");
    fs::create_dir_all(&export_path)?;
    export_path.push(format!("{}.{}", safe_name, format));

    let contents = if source_path.exists() {
        fs::read_to_string(source_path)?
    } else {
        match format.as_str() {
            "md" => format!(
                "# {}\n\nNo transcript has been generated yet.\n",
                session_name
            ),
            "txt" => format!(
                "{}\n\nNo transcript has been generated yet.\n",
                session_name
            ),
            "srt" => "1\n00:00:00,000 --> 00:00:02,000\nNo transcript has been generated yet.\n"
                .to_string(),
            "json" => serde_json::json!({ "sessionName": session_name, "source": "TableScribe" })
                .to_string(),
            _ => "Unsupported export format".to_string(),
        }
    };

    fs::write(&export_path, contents)?;
    Ok(export_path.display().to_string())
}
