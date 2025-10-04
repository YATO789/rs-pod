mod api;
use reqwest::Client;
use api::oauth::SpotifyOAuth;
use api::spotify::SpotifyClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let oauth = SpotifyOAuth::from_env(vec!["user-read-playback-state".to_string()])?;

    let access_token = oauth.get_spotify_access_token().await?;

    // 🎵 再生情報取得
    let spotify_client = SpotifyClient::new(Client::new(), access_token);
    spotify_client.display_current_playback().await?;

    Ok(())
}

