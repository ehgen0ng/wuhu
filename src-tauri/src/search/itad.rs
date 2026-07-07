use std::time::Duration;

use serde::Deserialize;

use crate::models::SteamSearchItem;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItadSearchGame {
    slug: String,
    title: String,
    #[serde(default)]
    assets: Option<serde_json::Value>,
}

pub(crate) async fn search_games(query: String) -> Result<Vec<SteamSearchItem>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    if contains_cjk(query) {
        return Ok(Vec::new());
    }

    let client = super::client("创建 IsThereAnyDeal 搜索请求失败")?;
    let response = send_search_request(&client, query).await?;

    let games = response
        .json::<Vec<ItadSearchGame>>()
        .await
        .map_err(|err| format!("解析 IsThereAnyDeal 搜索结果失败：{err}"))?;

    let mut tasks = Vec::new();
    for (index, game) in games.into_iter().take(10).enumerate() {
        let client = client.clone();
        tasks.push(tauri::async_runtime::spawn(async move {
            (index, resolve_search_game(client, game).await)
        }));
    }

    let mut indexed_items = Vec::new();
    for task in tasks {
        if let Ok((index, Some(item))) = task.await {
            indexed_items.push((index, item));
        }
    }
    indexed_items.sort_by_key(|(index, _)| *index);

    let mut items: Vec<SteamSearchItem> = Vec::new();
    for (_, item) in indexed_items {
        if !items.iter().any(|current| current.id == item.id) {
            items.push(item);
        }
    }
    Ok(items)
}

async fn send_search_request(
    client: &reqwest::Client,
    query: &str,
) -> Result<reqwest::Response, String> {
    let mut last_error = "未知错误".to_string();

    for attempt in 0..3 {
        match client
            .get("https://isthereanydeal.com/search/api/games/")
            .query(&[("q", query)])
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => return Ok(response),
            Ok(response) => {
                let status = response.status();
                last_error = format!("HTTP {status}");
                if !is_retryable_http_status(status) {
                    break;
                }
            }
            Err(err) => {
                last_error = err.to_string();
            }
        }

        if attempt < 2 {
            tokio::time::sleep(Duration::from_millis(350 * (attempt + 1))).await;
        }
    }

    Err(format!("IsThereAnyDeal 搜索失败：{last_error}"))
}

fn is_retryable_http_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.as_u16() >= 500
}

async fn resolve_search_game(
    client: reqwest::Client,
    game: ItadSearchGame,
) -> Option<SteamSearchItem> {
    let name = game.title.trim().to_string();
    if name.is_empty() || game.slug.trim().is_empty() {
        return None;
    }

    let response = client
        .get(format!(
            "https://isthereanydeal.com/game/{}/info/",
            game.slug.trim()
        ))
        .header(reqwest::header::ACCEPT, "text/html")
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }

    let html = response.text().await.ok()?;
    let id = extract_app_id(&html)?;

    Some(SteamSearchItem {
        item_type: "app".to_string(),
        name,
        id,
        tiny_image: asset_url(&game.assets),
        price: None,
        platforms: None,
    })
}

fn extract_app_id(html: &str) -> Option<u32> {
    [
        "https://store.steampowered.com/app/",
        "https://steamdb.info/app/",
        "https://www.protondb.com/app/",
        "/steam/apps/",
    ]
    .into_iter()
    .find_map(|marker| parse_digits_after(html, marker))
}

fn parse_digits_after(text: &str, marker: &str) -> Option<u32> {
    let start = text.find(marker)? + marker.len();
    let digits: String = text[start..]
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect();
    digits.parse::<u32>().ok().filter(|id| *id > 0)
}

fn asset_url(assets: &Option<serde_json::Value>) -> Option<String> {
    let object = assets.as_ref()?.as_object()?;
    ["boxart", "banner145", "banner300", "banner400"]
        .into_iter()
        .find_map(|key| object.get(key)?.as_str().map(ToString::to_string))
}

fn contains_cjk(text: &str) -> bool {
    text.chars().any(|character| {
        matches!(
            character as u32,
            0x3400..=0x4DBF
                | 0x4E00..=0x9FFF
                | 0xF900..=0xFAFF
                | 0x20000..=0x2A6DF
                | 0x2A700..=0x2B73F
                | 0x2B740..=0x2B81F
                | 0x2B820..=0x2CEAF
        )
    })
}
