use serde::Deserialize;

use crate::models::SteamSearchItem;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheapSharkGame {
    #[serde(rename = "steamAppID")]
    steam_app_id: Option<String>,
    external: Option<String>,
    thumb: Option<String>,
}

pub(crate) async fn search_games(query: String) -> Result<Vec<SteamSearchItem>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let client = super::client("创建 CheapShark 搜索请求失败")?;
    let response = client
        .get("https://www.cheapshark.com/api/1.0/games")
        .query(&[("title", query), ("limit", "10")])
        .send()
        .await
        .map_err(|err| format!("CheapShark 搜索请求失败：{err}"))?;

    if !response.status().is_success() {
        return Err(format!("CheapShark 搜索失败：HTTP {}", response.status()));
    }

    let games = response
        .json::<Vec<CheapSharkGame>>()
        .await
        .map_err(|err| format!("解析 CheapShark 搜索结果失败：{err}"))?;

    Ok(games
        .into_iter()
        .filter_map(|game| {
            let id = game.steam_app_id?.parse::<u32>().ok()?;
            let name = game.external?.trim().to_string();
            if id == 0 || name.is_empty() {
                return None;
            }

            Some(SteamSearchItem {
                item_type: "app".to_string(),
                name,
                id,
                tiny_image: game.thumb,
                price: None,
                platforms: None,
            })
        })
        .take(24)
        .collect())
}
