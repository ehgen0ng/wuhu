use std::time::Duration;

mod cheapshark;
mod itad;
mod steam;

pub(crate) use cheapshark::search_games as search_cheapshark_games;
pub(crate) use itad::search_games as search_isthereanydeal_games;
pub(crate) use steam::{
    search_games as search_steam_games, search_suggest_games as search_steam_suggest_games,
};

use crate::net::HTTP_USER_AGENT;

fn client(error_prefix: &str) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent(HTTP_USER_AGENT)
        .build()
        .map_err(|err| format!("{error_prefix}：{err}"))
}
