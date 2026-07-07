use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    detail: Option<String>,
    error: Option<String>,
    message: Option<String>,
}

pub(crate) async fn api_error_detail(response: reqwest::Response) -> Option<String> {
    response
        .text()
        .await
        .ok()
        .and_then(|text| parse_api_error_detail(&text))
}

pub(crate) fn parse_api_error_detail(text: &str) -> Option<String> {
    serde_json::from_str::<ApiErrorResponse>(text)
        .ok()
        .and_then(|body| body.detail.or(body.error).or(body.message))
        .filter(|message| !message.trim().is_empty())
        .or_else(|| {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.chars().take(180).collect())
            }
        })
}

pub(crate) fn detail_prefix(detail: &Option<String>) -> String {
    detail
        .as_deref()
        .map(|message| format!("：{message}"))
        .unwrap_or_default()
}

pub(crate) fn value_as_string(json: &serde_json::Value, key: &str) -> Option<String> {
    json.get(key).and_then(|value| match value {
        serde_json::Value::String(text) if !text.trim().is_empty() => Some(text.to_string()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        _ => None,
    })
}

pub(crate) fn value_as_bool(json: &serde_json::Value, key: &str) -> Option<bool> {
    json.get(key).and_then(|value| match value {
        serde_json::Value::Bool(value) => Some(*value),
        serde_json::Value::String(text) => text.parse().ok(),
        _ => None,
    })
}

pub(crate) fn value_as_u32(json: &serde_json::Value, key: &str) -> Option<u32> {
    value_as_u64(json, key).and_then(|value| u32::try_from(value).ok())
}

pub(crate) fn value_as_u64(json: &serde_json::Value, key: &str) -> Option<u64> {
    json.get(key).and_then(|value| match value {
        serde_json::Value::Number(number) => number.as_u64(),
        serde_json::Value::String(text) => text.parse().ok(),
        _ => None,
    })
}
