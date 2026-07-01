use crate::engine::models::DetectionConfig;

pub fn detection_config_from_settings(raw: &str) -> anyhow::Result<DetectionConfig> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return Ok(DetectionConfig::default());
    }

    let mut config: DetectionConfig = serde_json::from_str(trimmed)?;
    if config.folders.is_empty() {
        config.folders = DetectionConfig::default().folders;
    }
    if config.poll_interval_ms == 0 {
        config.poll_interval_ms = DetectionConfig::default().poll_interval_ms;
    }
    Ok(config)
}
