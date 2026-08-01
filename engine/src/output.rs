use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct OutputEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub module: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u128>,
}

impl OutputEvent {
    pub fn event(module: &str, level: &str, message: impl Into<String>) -> Self {
        OutputEvent {
            event_type: "event".to_string(),
            module: module.to_string(),
            level: Some(level.to_string()),
            message: Some(message.into()),
            data: None,
            current: None,
            total: None,
            duration_ms: None,
        }
    }

    pub fn progress(module: &str, current: u64, total: u64) -> Self {
        OutputEvent {
            event_type: "progress".to_string(),
            module: module.to_string(),
            level: None,
            message: None,
            data: None,
            current: Some(current),
            total: Some(total),
            duration_ms: None,
        }
    }

    pub fn result(module: &str, data: Value) -> Self {
        OutputEvent {
            event_type: "result".to_string(),
            module: module.to_string(),
            level: None,
            message: None,
            data: Some(data),
            current: None,
            total: None,
            duration_ms: None,
        }
    }

    pub fn complete(module: &str, duration_ms: u128) -> Self {
        OutputEvent {
            event_type: "complete".to_string(),
            module: module.to_string(),
            level: None,
            message: None,
            data: None,
            current: None,
            total: None,
            duration_ms: Some(duration_ms),
        }
    }
}

pub fn to_json_line(event: &OutputEvent) -> String {
    let mut line = serde_json::to_string(event).unwrap_or_default();
    line.push('\n');
    line
}
