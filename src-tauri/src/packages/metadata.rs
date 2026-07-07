pub(super) struct PackageMetadata {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) app_id: Option<u32>,
}

pub(crate) fn normalize_title(title: &str, app_id: u32) -> String {
    let title = title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    if title.is_empty() {
        app_id.to_string()
    } else {
        title
    }
}

pub(crate) fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let text = text.trim();
        if text.is_empty() {
            None
        } else {
            Some(text.to_string())
        }
    })
}

pub(super) fn parse_package_metadata(
    lua_file_name: &str,
    lua_content: &str,
    zip_stem: &str,
) -> PackageMetadata {
    let app_id = parse_app_id_from_text(lua_file_name)
        .or_else(|| parse_first_addappid(lua_content))
        .or_else(|| parse_app_id_from_text(zip_stem));
    let id = app_id
        .map(|value| value.to_string())
        .unwrap_or_else(|| zip_stem.to_string());
    let title = parse_title(lua_content).unwrap_or_else(|| id.clone());

    PackageMetadata { id, title, app_id }
}

pub(super) fn build_basic_lua(app_id: u32, title: &str) -> String {
    format!("-- {title}\naddappid({app_id})\n")
}

pub(super) fn sanitize_id(id: &str) -> String {
    id.chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .collect()
}

fn parse_first_addappid(text: &str) -> Option<u32> {
    let marker = "addappid";
    let lower = text.to_ascii_lowercase();
    let start = lower.find(marker)?;
    let rest = &text[start + marker.len()..];
    let open = rest.find('(')?;
    let after_open = &rest[open + 1..];
    let digits: String = after_open
        .chars()
        .skip_while(|ch| ch.is_whitespace())
        .take_while(|ch| ch.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

fn parse_app_id_from_text(text: &str) -> Option<u32> {
    let digits: String = text.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

fn parse_title(lua_content: &str) -> Option<String> {
    for line in lua_content.lines() {
        let trimmed = line.trim_start();
        let Some(comment) = trimmed.strip_prefix("--") else {
            continue;
        };
        let title = comment.trim();
        if title.is_empty() || is_metadata_comment(title) {
            continue;
        }
        return Some(title.to_string());
    }
    None
}

fn is_metadata_comment(comment: &str) -> bool {
    let lower = comment.to_ascii_lowercase();
    lower.contains("lua and manifest")
        || lower.starts_with("created")
        || lower.starts_with("website")
        || lower.starts_with("total")
        || lower.starts_with("main")
        || lower.contains("depot")
}
