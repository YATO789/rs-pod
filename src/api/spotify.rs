use reqwest::Client;
use serde::Deserialize;

pub enum SkipDirection {
    Next,
    Previous,
}

pub struct SpotifyClient {
    client: Client,
    access_token: String,
    pub spotify_player : SpotifyPlayer,
}

#[derive(Deserialize, Debug)]
pub struct SpotifyPlayer {
    pub is_playing: bool,
    pub item: Option<Track>,
    pub progress_ms: Option<i64>,
}

#[derive(Deserialize, Debug)]
pub struct Track {
    pub name: String,
    pub artists: Vec<Artist>,
    pub duration_ms: i64,
}

#[derive(Deserialize, Debug)]
pub struct Artist {
    pub name: String,
}

impl Default for SpotifyPlayer {
    fn default() -> Self {
        Self {
            is_playing: false,
            item: None,
            progress_ms: None,
        }
    }
}

impl SpotifyClient {
    pub fn new(client: Client, access_token: &String) -> Self {
        Self {
            spotify_player: SpotifyPlayer::default(),
            client,
            access_token: access_token.to_string(),
        }
    }

    pub async fn init(mut self) -> Result<Self, Box<dyn std::error::Error>> {
        self.spotify_player = self.get_current_playback().await?;
        Ok(self)
    }

    pub async fn get_current_playback(&self) -> Result<SpotifyPlayer, Box<dyn std::error::Error>> {
        let res = self.client
            .get("https://api.spotify.com/v1/me/player")
            .bearer_auth(&self.access_token)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(format!("Failed to fetch player info: {}", res.status()).into());
        }

        let player: SpotifyPlayer = res.json().await?;
        Ok(player)
    }

    pub async fn skip_track(&mut self, direction: SkipDirection) -> Result<(), Box<dyn std::error::Error>> {
        let endpoint = match direction {
            SkipDirection::Next => "https://api.spotify.com/v1/me/player/next",
            SkipDirection::Previous => "https://api.spotify.com/v1/me/player/previous",
        };

        let res = self.client
            .post(endpoint)
            .bearer_auth(&self.access_token)
            .header("Content-Length", "0")
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(format!("Failed to skip track: {}", res.status()).into());
        }

        // Update current playback info
        self.spotify_player = self.get_current_playback().await?;
        Ok(())
    }
}
