//! Discord OAuth2 — the only file that talks to Discord. Confidential-client
//! authorization-code flow, `identify` scope only (the guild list joins the
//! scopes when communities land, v3 in the design doc).

use anyhow::Context;

pub fn authorize_url(client_id: &str, redirect_uri: &str, state: &str) -> String {
    format!(
        "https://discord.com/oauth2/authorize?response_type=code&client_id={}&scope=identify&state={}&redirect_uri={}",
        client_id,
        state,
        urlencode(redirect_uri),
    )
}

pub async fn exchange_code(
    http: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
) -> anyhow::Result<String> {
    #[derive(serde::Deserialize)]
    struct TokenResponse {
        access_token: String,
    }

    let response = http
        .post("https://discord.com/api/oauth2/token")
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
        ])
        .send()
        .await
        .context("discord token request failed")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("discord token exchange failed ({status}): {body}");
    }
    Ok(response.json::<TokenResponse>().await?.access_token)
}

#[derive(serde::Deserialize)]
pub struct DiscordUser {
    pub id: String,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
}

impl DiscordUser {
    /// Discord's display name — `global_name` with the legacy `username` as
    /// fallback (bots and old accounts may lack a global name).
    pub fn display_name(&self) -> &str {
        self.global_name.as_deref().unwrap_or(&self.username)
    }
}

pub async fn fetch_user(http: &reqwest::Client, access_token: &str) -> anyhow::Result<DiscordUser> {
    let response = http
        .get("https://discord.com/api/users/@me")
        .bearer_auth(access_token)
        .send()
        .await
        .context("discord /users/@me failed")?;
    if !response.status().is_success() {
        anyhow::bail!("discord /users/@me returned {}", response.status());
    }
    Ok(response.json().await?)
}

/// Percent-encode the redirect URI (the only param that needs it here).
fn urlencode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}
