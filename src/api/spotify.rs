use reqwest::Client;
use serde::Deserialize;

pub struct SpotifyClient {
    client: Client,
    access_token: String,
}

#[derive(Deserialize, Debug)]
pub struct SpotifyPlayer {
    pub is_playing: bool,
    pub item: Option<Track>,
}

#[derive(Deserialize, Debug)]
pub struct Track {
    pub name: String,
    pub artists: Vec<Artist>,
}

#[derive(Deserialize, Debug)]
pub struct Artist {
    pub name: String,
}

impl SpotifyClient {
    pub fn new(client: Client, access_token: String) -> Self {
        Self {
            client,
            access_token,
        }
    }

    pub async fn display_current_playback(&self) -> Result<(), Box<dyn std::error::Error>> {
        let res = self.client
            .get("https://api.spotify.com/v1/me/player")
            .bearer_auth(&self.access_token)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(format!("Failed to fetch player info: {}", res.status()).into());
        }

        let player: SpotifyPlayer = res.json().await?;

        println!("\n=== 🎧 再生情報 ===");
        if player.is_playing {
            if let Some(track) = &player.item {
                let artists: Vec<String> = track.artists.iter().map(|a| a.name.clone()).collect();
                println!("Now Playing: {} - {}", track.name, artists.join(", "));
            } else {
                println!("曲情報を取得できませんでした。");
            }
        } else {
            println!("現在は再生中ではありません。");
        }

        Ok(())
    }
}