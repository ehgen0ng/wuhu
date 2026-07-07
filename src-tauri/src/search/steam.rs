use serde::Deserialize;

use crate::models::{SteamSearchItem, SteamSearchPrice};

#[derive(Debug, Deserialize)]
struct SteamSearchResponse {
    items: Vec<SteamSearchItem>,
}

pub(crate) async fn search_games(query: String) -> Result<Vec<SteamSearchItem>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let client = super::client("创建 Steam 搜索请求失败")?;
    let response = client
        .get("https://store.steampowered.com/api/storesearch/")
        .query(&[("term", query), ("l", "schinese"), ("cc", "cn")])
        .send()
        .await
        .map_err(|err| format!("Steam 搜索请求失败：{err}"))?;

    if !response.status().is_success() {
        return Err(format!("Steam 搜索失败：HTTP {}", response.status()));
    }

    let body = response
        .json::<SteamSearchResponse>()
        .await
        .map_err(|err| format!("解析 Steam 搜索结果失败：{err}"))?;
    Ok(body
        .items
        .into_iter()
        .filter(|item| item.item_type == "app" && !item.name.trim().is_empty())
        .take(24)
        .collect())
}

pub(crate) async fn search_suggest_games(query: String) -> Result<Vec<SteamSearchItem>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let client = super::client("创建 Steam 搜索建议请求失败")?;
    let response = client
        .get("https://store.steampowered.com/search/suggest")
        .query(&[
            ("term", query),
            ("f", "games"),
            ("cc", "cn"),
            ("realm", "1"),
            ("l", "schinese"),
        ])
        .send()
        .await
        .map_err(|err| format!("Steam 搜索建议请求失败：{err}"))?;

    if !response.status().is_success() {
        return Err(format!("Steam 搜索建议失败：HTTP {}", response.status()));
    }

    let html = response
        .text()
        .await
        .map_err(|err| format!("读取 Steam 搜索建议失败：{err}"))?;
    Ok(parse_suggest_items(&html))
}

fn parse_suggest_items(html: &str) -> Vec<SteamSearchItem> {
    html.split("<a ")
        .skip(1)
        .filter_map(parse_suggest_anchor)
        .take(24)
        .collect()
}

fn parse_suggest_anchor(anchor: &str) -> Option<SteamSearchItem> {
    let id = find_html_attr(anchor, "data-ds-appid")?
        .parse::<u32>()
        .ok()?;
    let name = extract_between(anchor, "<div class=\"match_name\">", "</div>")
        .map(decode_html_text)
        .map(|value| value.trim().to_string())?;

    if id == 0 || name.is_empty() {
        return None;
    }

    let tiny_image = extract_between(anchor, "<div class=\"match_img\">", "</div>")
        .and_then(|image_html| find_html_attr(image_html, "src"));
    let price = extract_between(anchor, "<div class=\"match_price\">", "</div>")
        .map(decode_html_text)
        .and_then(|text| parse_cny_price(&text));

    Some(SteamSearchItem {
        item_type: "app".to_string(),
        name,
        id,
        tiny_image,
        price,
        platforms: None,
    })
}

fn find_html_attr(html: &str, attr: &str) -> Option<String> {
    let marker = format!("{attr}=\"");
    let start = html.find(&marker)? + marker.len();
    let rest = &html[start..];
    let end = rest.find('"')?;
    Some(decode_html_text(&rest[..end]))
}

fn extract_between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let start_index = text.find(start)? + start.len();
    let rest = &text[start_index..];
    let end_index = rest.find(end)?;
    Some(&rest[..end_index])
}

fn decode_html_text(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

fn parse_cny_price(text: &str) -> Option<SteamSearchPrice> {
    let normalized: String = text
        .chars()
        .filter(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect();
    if normalized.is_empty() {
        return None;
    }

    let value = normalized.parse::<f64>().ok()?;
    Some(SteamSearchPrice {
        currency: "CNY".to_string(),
        initial: (value * 100.0).round() as u32,
        final_price: (value * 100.0).round() as u32,
    })
}
