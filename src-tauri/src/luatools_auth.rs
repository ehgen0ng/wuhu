use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    sync::OnceLock,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

use crate::{net::HTTP_USER_AGENT, store::portable_data_dir};

pub(crate) const API_URL: &str = "https://lua.tools";
pub(crate) const AUTH_URL: &str = "https://db.lua.tools";
// Public client configuration, also shipped in LuaTools' AppConfig.cs and web bundle.
pub(crate) const ANON_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpYXQiOjE3NzYwMzkzNzYsImV4cCI6MTg5MzQ1NjAwMCwicm9sZSI6ImFub24iLCJpc3MiOiJzdXBhYmFzZSJ9.f_-K38u3odjltP-g_67FVmG32Vg-_-k-lNBvIaVUVBM";
const SESSION_FILE: &str = "luatools-session.json";
const LOGIN_REQUIRED: &str = "LuaTools 登录已失效，请重新输入 Cookie 或令牌";
const INVALID_INPUT: &str =
    "无法解析 LuaTools 凭证，请输入 Cookie、refresh_token、access_token 或会话 JSON";
static SESSION_LOCK: Mutex<()> = Mutex::const_new(());
static APP: OnceLock<AppHandle> = OnceLock::new();

#[derive(Clone, Serialize, Deserialize, Default)]
struct Session {
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    expires_at: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthState {
    pub(crate) credential: String,
    pub(crate) has_refresh_token: bool,
    pub(crate) expires_at: u64,
}

impl Session {
    fn state(&self) -> AuthState {
        AuthState {
            credential: if self.refresh_token.is_empty() {
                self.access_token.clone()
            } else {
                self.refresh_token.clone()
            },
            has_refresh_token: !self.refresh_token.is_empty(),
            expires_at: self.expires_at,
        }
    }

    fn needs_refresh(&self, now: u64) -> bool {
        self.access_token.is_empty()
            || self.expires_at
                <= now.saturating_add(if self.refresh_token.is_empty() {
                    0
                } else {
                    120
                })
    }
}

pub(crate) fn init(app: AppHandle) {
    let _ = APP.set(app);
}

pub(crate) fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(HTTP_USER_AGENT)
        .build()
        .map_err(|_| "创建 LuaTools 请求失败".to_string())
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn read_session() -> Result<Option<Session>, String> {
    let path = portable_data_dir()?.join(SESSION_FILE);
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| "读取 LuaTools 登录信息失败".to_string()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("读取 LuaTools 登录信息失败".to_string()),
    }
}

pub(crate) fn state() -> Result<Option<AuthState>, String> {
    Ok(read_session()?.map(|session| session.state()))
}

fn persist(session: Option<&Session>) -> Result<(), String> {
    let root = portable_data_dir()?;
    let path = root.join(SESSION_FILE);
    if let Some(session) = session {
        let bytes =
            serde_json::to_vec(session).map_err(|_| "保存 LuaTools 登录信息失败".to_string())?;
        let pending = root.join("luatools-session.pending");
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let result = (|| -> std::io::Result<()> {
            let mut file = options.open(&pending)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&pending, &path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(pending);
            return Err("保存 LuaTools 登录信息失败".to_string());
        }
    } else if let Err(err) = fs::remove_file(path) {
        if err.kind() != std::io::ErrorKind::NotFound {
            return Err("保存 LuaTools 登录信息失败".to_string());
        }
    }
    if let Some(app) = APP.get() {
        let _ = app.emit("luatools-auth-changed", session.map(Session::state));
    }
    Ok(())
}

fn decode_base64(value: &str) -> Result<Vec<u8>, String> {
    general_purpose::URL_SAFE_NO_PAD
        .decode(value.trim_end_matches('='))
        .or_else(|_| general_purpose::STANDARD.decode(value))
        .map_err(|_| INVALID_INPUT.to_string())
}

fn token_expiry(token: &str) -> Result<u64, String> {
    let parts: Vec<_> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(INVALID_INPUT.to_string());
    }
    let payload: serde_json::Value =
        serde_json::from_slice(&decode_base64(parts[1])?).map_err(|_| INVALID_INPUT.to_string())?;
    // This is only used for scheduling. The service validates the token before it is saved.
    payload
        .get("exp")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| INVALID_INPUT.to_string())
}

fn session_from_json(value: &str) -> Result<Session, String> {
    let mut session: Session =
        serde_json::from_str(value).map_err(|_| INVALID_INPUT.to_string())?;
    session.access_token = session.access_token.trim().to_string();
    session.refresh_token = session.refresh_token.trim().to_string();
    if session.access_token.is_empty() && session.refresh_token.is_empty() {
        return Err(INVALID_INPUT.to_string());
    }
    if !session.access_token.is_empty() {
        session.expires_at = token_expiry(&session.access_token)?;
    }
    Ok(session)
}

fn parse_input(input: &str) -> Result<Session, String> {
    let input = input.trim();
    if input.len() > 128 * 1024 {
        return Err(INVALID_INPUT.to_string());
    }
    if input.starts_with('{') {
        return session_from_json(input);
    }
    if let Some(start) = input.find("sb-db-auth-token") {
        let mut whole = None;
        let mut chunks = BTreeMap::new();
        for entry in input[start..].trim_matches(['\'', '"']).split(';') {
            let Some((name, value)) = entry.trim().split_once('=') else {
                continue;
            };
            let value = value.trim().trim_matches(['\'', '"']);
            if name == "sb-db-auth-token" {
                whole = Some(value.to_string());
            } else if let Some(index) = name
                .strip_prefix("sb-db-auth-token.")
                .and_then(|s| s.parse::<usize>().ok())
            {
                if chunks.insert(index, value.to_string()).is_some() {
                    return Err(INVALID_INPUT.to_string());
                }
            }
        }
        let encoded = if let Some(value) = whole {
            value
        } else {
            if chunks.is_empty() || chunks.keys().copied().ne(0..chunks.len()) {
                return Err(INVALID_INPUT.to_string());
            }
            chunks.into_values().collect::<String>()
        };
        let encoded = encoded
            .strip_prefix("base64-")
            .ok_or_else(|| INVALID_INPUT.to_string())?;
        let json =
            String::from_utf8(decode_base64(encoded)?).map_err(|_| INVALID_INPUT.to_string())?;
        return session_from_json(&json);
    }
    let token = input
        .strip_prefix("Bearer ")
        .or_else(|| input.strip_prefix("bearer "))
        .unwrap_or(input);
    if token.is_empty()
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    {
        return Err(INVALID_INPUT.to_string());
    }
    if token.contains('.') {
        Ok(Session {
            access_token: token.to_string(),
            expires_at: token_expiry(token)?,
            ..Session::default()
        })
    } else {
        Ok(Session {
            refresh_token: token.to_string(),
            ..Session::default()
        })
    }
}

pub(crate) fn check_response(response: &reqwest::Response) -> Result<(), String> {
    match response.status() {
        status if status.is_success() => Ok(()),
        reqwest::StatusCode::UNAUTHORIZED => Err(LOGIN_REQUIRED.to_string()),
        reqwest::StatusCode::FORBIDDEN => {
            Err("LuaTools 拒绝访问，请检查账号权限或稍后重试".to_string())
        }
        reqwest::StatusCode::TOO_MANY_REQUESTS => {
            Err("LuaTools 请求过快或当天次数已用完，请稍后再试".to_string())
        }
        status => Err(format!("LuaTools 请求失败：HTTP {}", status.as_u16())),
    }
}

async fn refresh(session: &Session) -> Result<Session, String> {
    if session.refresh_token.is_empty() {
        return Err("LuaTools access_token 已过期，请提供 refresh_token 或新的 Cookie".to_string());
    }
    let response = client()?
        .post(format!("{AUTH_URL}/auth/v1/token?grant_type=refresh_token"))
        .header("apikey", ANON_KEY)
        .json(&serde_json::json!({ "refresh_token": session.refresh_token }))
        .send()
        .await
        .map_err(|_| "LuaTools 网络请求失败，请稍后重试".to_string())?;
    if matches!(
        response.status(),
        reqwest::StatusCode::BAD_REQUEST | reqwest::StatusCode::UNAUTHORIZED
    ) {
        return Err(LOGIN_REQUIRED.to_string());
    }
    check_response(&response)?;
    let mut refreshed: Session = response
        .json()
        .await
        .map_err(|_| "解析 LuaTools 会话失败".to_string())?;
    if refreshed.access_token.is_empty() || refreshed.refresh_token.is_empty() {
        return Err("解析 LuaTools 会话失败".to_string());
    }
    refreshed.expires_at = token_expiry(&refreshed.access_token)?;
    Ok(refreshed)
}

pub(crate) async fn save_credentials(input: &str) -> Result<(), String> {
    let _guard = SESSION_LOCK.lock().await;
    if input.trim().is_empty() {
        return persist(None);
    }
    let mut session = parse_input(input)?;
    // Clicking Save on the displayed token must not rotate a perfectly valid session again.
    if session.access_token.is_empty() {
        if let Some(current) = read_session()? {
            if session.refresh_token == current.refresh_token {
                session = current;
            }
        }
    }
    if session.needs_refresh(now_seconds()) {
        // The auth server already validated this refresh. Save its rotated token immediately.
        session = refresh(&session).await?;
        return persist(Some(&session));
    }
    let response = client()?
        .get(format!("{AUTH_URL}/auth/v1/user"))
        .header("apikey", ANON_KEY)
        .bearer_auth(&session.access_token)
        .send()
        .await
        .map_err(|_| "LuaTools 网络请求失败，请稍后重试".to_string())?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED && !session.refresh_token.is_empty() {
        session = refresh(&session).await?;
    } else {
        check_response(&response)?;
    }
    persist(Some(&session))
}

async fn access_token(rejected: Option<&str>) -> Result<String, String> {
    let _guard = SESSION_LOCK.lock().await;
    let mut session =
        read_session()?.ok_or_else(|| "请先在设置里保存 LuaTools Cookie 或令牌".to_string())?;
    if session.needs_refresh(now_seconds()) || rejected == Some(session.access_token.as_str()) {
        session = refresh(&session).await?;
        // Persist both tokens before releasing the lock or sending another authenticated request.
        persist(Some(&session))?;
    }
    Ok(session.access_token)
}

pub(crate) async fn send(request: reqwest::RequestBuilder) -> Result<reqwest::Response, String> {
    let retry = request
        .try_clone()
        .ok_or_else(|| "创建 LuaTools 请求失败".to_string())?;
    let token = access_token(None).await?;
    let response = request
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|_| "LuaTools 网络请求失败，请稍后重试".to_string())?;
    if response.status() != reqwest::StatusCode::UNAUTHORIZED {
        return Ok(response);
    }
    let token = access_token(Some(&token)).await?;
    retry
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| "LuaTools 网络请求失败，请稍后重试".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jwt(exp: u64) -> String {
        format!(
            "header.{}.signature",
            general_purpose::URL_SAFE_NO_PAD.encode(format!("{{\"exp\":{exp}}}"))
        )
    }

    #[test]
    fn accepts_chunked_cookie_in_numeric_order_and_drops_unrelated_fields() {
        let access = jwt(123456);
        let json = serde_json::json!({"access_token":access,"refresh_token":"refreshA","provider_token":"must-not-persist","user":{"email":"private@example.invalid"}}).to_string();
        let encoded = format!("base64-{}", general_purpose::URL_SAFE_NO_PAD.encode(json));
        let (first, second) = encoded.split_at(37);
        let input =
            format!("Cookie: sb-db-auth-token.1={second}; unrelated=x; sb-db-auth-token.0={first}");
        let session = parse_input(&input).unwrap();
        assert_eq!(session.access_token, access);
        assert_eq!(session.refresh_token, "refreshA");
        assert_eq!(session.expires_at, 123456);
        assert!(!serde_json::to_string(&session).unwrap().contains("private"));
        assert!(!serde_json::to_string(&session)
            .unwrap()
            .contains("must-not-persist"));
    }

    #[test]
    fn accepts_whole_cookie_json_and_individual_tokens() {
        let access = jwt(300);
        let json = serde_json::json!({"access_token":access,"refresh_token":"refreshA","expires_at":999999}).to_string();
        let cookie = format!(
            "sb-db-auth-token=base64-{}",
            general_purpose::URL_SAFE_NO_PAD.encode(&json)
        );
        assert_eq!(parse_input(&cookie).unwrap().expires_at, 300);
        assert_eq!(parse_input(&json).unwrap().refresh_token, "refreshA");
        assert!(parse_input(&format!("Bearer {access}"))
            .unwrap()
            .refresh_token
            .is_empty());
        assert_eq!(parse_input("refreshA").unwrap().refresh_token, "refreshA");
    }

    #[test]
    fn rejects_incomplete_cookies_and_invalid_values_without_echoing_them() {
        for input in [
            "sb-db-auth-token.1=secret",
            "sb-db-auth-token.0=secret; sb-db-auth-token.2=secret",
            "{\"access_token\":\"secret\"}",
            "{}",
            "bad secret",
            "x.y.z",
        ] {
            let error = parse_input(input).err().unwrap();
            assert_eq!(error, INVALID_INPUT);
        }
    }

    #[test]
    fn access_only_tokens_remain_usable_until_expiry_and_refreshable_tokens_refresh_early() {
        let mut session = Session {
            access_token: jwt(300),
            expires_at: 300,
            ..Session::default()
        };
        assert!(!session.needs_refresh(299));
        assert!(session.needs_refresh(300));
        session.refresh_token = "refreshA".to_string();
        assert!(!session.needs_refresh(179));
        assert!(session.needs_refresh(180));
        assert_eq!(session.state().credential, "refreshA");
    }
}
