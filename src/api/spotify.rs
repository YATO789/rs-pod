use reqwest::Client;
use serde::Deserialize;
use image::{DynamicImage, ImageReader};
use std::io::Cursor;

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
    pub album: Album,
}

#[derive(Deserialize, Debug)]
pub struct Album {
    pub images: Vec<Image>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Image {
    pub url: String,
    pub height: Option<i32>,
    pub width: Option<i32>,
}

#[derive(Deserialize, Debug)]
pub struct Artist {
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct PlaylistsResponse {
    pub items: Vec<Playlist>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub tracks: PlaylistTracks,
    pub images: Vec<Image>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PlaylistTracks {
    pub total: i32,
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
            .query(&[("market", "US")])
            .send()
            .await?;

        // 204 No Content: 何も再生していない場合
        if res.status().as_u16() == 204 {
            return Ok(SpotifyPlayer::default());
        }

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

    pub async fn download_image(&self, url: &str) -> Result<DynamicImage, Box<dyn std::error::Error>> {
        // URLから画像を取得
        let bytes = self.client.get(url).send().await?.bytes().await?;

        // image crate でデコード
        let dyn_img = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()?
            .decode()?;

        Ok(dyn_img)
    }

    pub async fn get_user_playlists(&self) -> Result<Vec<Playlist>, Box<dyn std::error::Error>> {
        let res = self.client
            .get("https://api.spotify.com/v1/me/playlists")
            .bearer_auth(&self.access_token)
            .query(&[("limit", "50"), ("market", "US")])
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(format!("Failed to fetch playlists: {}", res.status()).into());
        }

        let playlists: PlaylistsResponse = res.json().await?;
        Ok(playlists.items)
    }

    pub async fn play_playlist(&self, playlist_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let body = serde_json::json!({
            "context_uri": format!("spotify:playlist:{}", playlist_id),
            "offset": {
                "position": 0
            },
            "position_ms": 0
        });

        let res = self.client
            .put("https://api.spotify.com/v1/me/player/play")
            .bearer_auth(&self.access_token)
            .json(&body)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(format!("Failed to play playlist: {}", res.status()).into());
        }

        Ok(())
    }
}
