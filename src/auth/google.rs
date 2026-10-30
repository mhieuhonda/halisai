use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

const JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";
const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

#[derive(Clone, Debug)]
pub struct GoogleIdentity {
    pub sub: String,
    pub email: String,
    pub name: String,
    pub picture: Option<String>,
}

#[derive(Deserialize)]
pub struct TokenResponse {
    pub id_token: String,
}

#[derive(Deserialize)]
struct GoogleClaims {
    sub: String,
    email: String,
    email_verified: Option<bool>,
    name: Option<String>,
    picture: Option<String>,
}

pub fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn authorize_url(client_id: &str, redirect_uri: &str, state: &str) -> String {
    format!(
        "{AUTH_URL}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}&prompt=select_account",
        url_encode(client_id),
        url_encode(redirect_uri),
        url_encode("openid email profile"),
        url_encode(state),
    )
}

pub async fn exchange_code(
    http: &reqwest::Client,
    code: &str,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
) -> Result<TokenResponse, String> {
    let params = [
        ("code", code),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("redirect_uri", redirect_uri),
        ("grant_type", "authorization_code"),
    ];
    let resp = http.post(TOKEN_URL).form(&params).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let status = resp.status();
        tracing::warn!("google token exchange loi {status}");
        return Err("khong doi duoc token".into());
    }
    resp.json::<TokenResponse>().await.map_err(|e| e.to_string())
}

static JWKS_CACHE: LazyLock<Mutex<Option<(Instant, JwkSet)>>> =
    LazyLock::new(|| Mutex::new(None));

async fn fetch_jwks(http: &reqwest::Client) -> Result<JwkSet, String> {
    if let Some((at, set)) = JWKS_CACHE.lock().unwrap().clone() {
        if at.elapsed() < Duration::from_secs(3600) {
            return Ok(set);
        }
    }
    let set = http
        .get(JWKS_URL)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<JwkSet>()
        .await
        .map_err(|e| e.to_string())?;
    *JWKS_CACHE.lock().unwrap() = Some((Instant::now(), set.clone()));
    Ok(set)
}

/// Xac thuc chu ky id_token voi JWKS cua Google.
pub async fn verify_id_token(
    http: &reqwest::Client,
    id_token: &str,
    client_id: &str,
) -> Result<GoogleIdentity, String> {
    let header = jsonwebtoken::decode_header(id_token).map_err(|e| e.to_string())?;
    let kid = header.kid.ok_or("thieu kid")?;
    let set = fetch_jwks(http).await?;
    let jwk = set.find(&kid).ok_or("khong tim thay key")?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[client_id]);
    validation.iss = Some(
        ["https://accounts.google.com", "accounts.google.com"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
    );
    let data = decode::<GoogleClaims>(id_token, &DecodingKey::from_jwk(jwk).map_err(|e| e.to_string())?, &validation)
        .map_err(|e| format!("verify that bai: {e}"))?;

    let c = data.claims;
    if c.email_verified != Some(true) {
        return Err("email chua xac thuc".into());
    }
    Ok(GoogleIdentity {
        sub: c.sub,
        email: c.email,
        name: c.name.unwrap_or_else(|| "Người dùng".into()),
        picture: c.picture,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("a b&c"), "a%20b%26c");
        assert_eq!(url_encode("http://x/y"), "http%3A%2F%2Fx%2Fy");
    }
}
