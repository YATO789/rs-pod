use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{env, fs, path::Path};
use rand::{distributions::Alphanumeric, Rng};
use tiny_http::{Server, Response};
use url::Url;

const TOKEN_FILE: &str = "spotify_token.json";

#[derive(Debug, Clone)]
pub struct SpotifyOAuth {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    scopes: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
    refresh_token: Option<String>,
}

impl SpotifyOAuth {
    pub fn from_env(scopes: Vec<String>) -> Result<Self, Box<dyn std::error::Error>> {

        dotenv::dotenv().ok();
        
        let client_id = env::var("CLIENT_ID")?;
        let client_secret = env::var("CLIENT_SECRET")?;
        let redirect_uri = env::var("REDIRECT_URI")?;

        Ok(Self {
            client_id,
            client_secret,
            redirect_uri,
            scopes,
        })
    }

    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        redirect_uri: impl Into<String>,
        scopes: Vec<String>,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            redirect_uri: redirect_uri.into(),
            scopes,
        }
    }

    /// 🎫 Spotifyトークンを取得（リフレッシュ対応）
    pub async fn get_spotify_access_token(
        &self,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // すでにトークンファイルが存在する場合
        if Path::new(TOKEN_FILE).exists() {
            let json = fs::read_to_string(TOKEN_FILE)?;
            let token_data: TokenResponse = serde_json::from_str(&json)?;

            // refresh_token がある場合は再利用
            if let Some(refresh_token) = &token_data.refresh_token {
                println!("🔄 Refreshing access token...");

                let client = Client::new();
                let params = [
                    ("grant_type", "refresh_token"),
                    ("refresh_token", refresh_token.as_str()),
                    ("client_id", self.client_id.as_str()),
                    ("client_secret", self.client_secret.as_str()),
                ];

                let res = client
                    .post("https://accounts.spotify.com/api/token")
                    .form(&params)
                    .send()
                    .await?;

                if res.status().is_success() {
                    let new_token: TokenResponse = res.json().await?;
                    println!("✅ Access token refreshed.");

                    // refresh_token が返ってこない場合もあるので既存のものを保持
                    let merged_token = TokenResponse {
                        refresh_token: Some(refresh_token.clone()),
                        ..new_token
                    };

                    fs::write(TOKEN_FILE, serde_json::to_string_pretty(&merged_token)?)?;
                    return Ok(merged_token.access_token);
                } else {
                    println!("⚠️ Refresh token invalid, doing full auth again...");
                }
            }
        }

        // ⛳ 初回認証（または refresh_token 失効時）
        println!("🌐 Performing new authorization...");
        let new_token = Self::authorize_spotify(
            &self.client_id,
            &self.client_secret,
            &self.redirect_uri,
            &self.scopes,
        )
        .await?;

        fs::write(TOKEN_FILE, serde_json::to_string_pretty(&new_token)?)?;
        Ok(new_token.access_token)
    }

    /// 🧭 Spotify OAuth 認証（初回のみ実行）
    async fn authorize_spotify(
        client_id: &str,
        client_secret: &str,
        redirect_uri: &str,
        scopes: &[String],
    ) -> Result<TokenResponse, Box<dyn std::error::Error>> {
        // 1️⃣ state生成
        let state: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();

        // 2️⃣ 認可URL作成
        let mut auth_url = Url::parse("https://accounts.spotify.com/authorize")?;
        auth_url
            .query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", client_id)
            .append_pair("scope", &scopes.join(" "))
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("state", &state);

        println!("🔗 Open this URL in your browser:\n{}", auth_url);
        webbrowser::open(auth_url.as_str()).ok();

        // 3️⃣ localhost:8888 で待機して code を受け取る
        println!("Waiting for Spotify redirect...");
        let server = Server::http("0.0.0.0:8888").expect("");

        let mut code = None;
        for request in server.incoming_requests() {
            if request.url().starts_with("/callback") {
                let url = format!("http://localhost:8888{}", request.url());
                let parsed = Url::parse(&url)?;
                if let Some(query_code) = parsed.query_pairs().find(|(k, _)| k == "code") {
                    code = Some(query_code.1.to_string());
                }

                let response = Response::from_string("✅ 認証が完了しました！アプリに戻ってください。");
                request.respond(response)?;
                break;
            }
        }

        let code = code.ok_or("authorization code not found")?;
        println!("Got authorization code: {}", code);

        // 4️⃣ アクセストークン取得
        let client = Client::new();
        let params = [
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", redirect_uri),
            ("client_id", client_id),
            ("client_secret", client_secret),
        ];

        let res = client
            .post("https://accounts.spotify.com/api/token")
            .form(&params)
            .send()
            .await?;

        let token_json: TokenResponse = res.json().await?;
        println!("✅ Access token acquired!");
        Ok(token_json)
    }
}
