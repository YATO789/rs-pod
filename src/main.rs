mod api;
use std::env;
use reqwest::Client;
use api::oauth::SpotifyOAuth;
use api::spotify::SpotifyClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();

    let client_id = env::var("CLIENT_ID")?;
    let client_secret = env::var("CLIENT_SECRET")?;
    let redirect_uri = env::var("REDIRECT_URI")?;


    let oauth = SpotifyOAuth::new(
        client_id,
        client_secret,
        redirect_uri,
        vec!["user-read-playback-state".to_string()],
    );

    let access_token = oauth.get_spotify_access_token().await?;

    // 🎵 再生情報取得
    let spotify_client = SpotifyClient::new(Client::new(), access_token);
    spotify_client.display_current_playback().await?;

    Ok(())
}

