mod api;
use api::oauth::SpotifyOAuth;
use color_eyre::Result;
use ratatui::{
    buffer::Buffer,
    layout::{Rect, Layout, Constraint, Direction},
    style::{Stylize, Style, Color},
    text::Line,
    widgets::{Paragraph, Widget, Gauge},
    DefaultTerminal, Frame,
};
use crossterm::{event::{self, Event, KeyCode, KeyEvent, KeyEventKind}};
use std::{io, time::Duration};
use reqwest::Client;
use crate::api::spotify::{SpotifyClient, SkipDirection};

// ミリ秒をmm:ss形式にフォーマット
fn format_time(ms: i64) -> String {
    let total_seconds = ms / 1000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{}:{:02}", minutes, seconds)
}


#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let mut terminal = ratatui::init();
    terminal.clear()?; // 初回だけクリア
    let app_result = App::new().await?.run(&mut terminal).await;
    ratatui::restore();
    app_result
}


struct App{
    spotify_client :SpotifyClient,
    exit: bool,
}

impl App{
    async fn new() -> Result<Self>{
        //1. get oauth
        let access_token = SpotifyOAuth::init().await
            .map_err(|e| color_eyre::eyre::eyre!("{}", e))?;

        // init spotify_client and get current song
        let spotify_client = SpotifyClient::new(Client::new(), &access_token)
            .init()
            .await
            .map_err(|e| color_eyre::eyre::eyre!("{}", e))?;

        Ok(Self{spotify_client,exit : false})
    }

    async fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()>{
        let mut last_update = tokio::time::Instant::now();
        let update_interval = Duration::from_secs(1);

        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;

            // 非ブロッキングでイベントを処理
            self.handle_events().await?;

            // 1秒ごとに更新
            if last_update.elapsed() >= update_interval {
                if let Ok(player) = self.spotify_client.get_current_playback().await {
                    self.spotify_client.spotify_player = player;
                }
                last_update = tokio::time::Instant::now();
            }
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

        async fn handle_events(&mut self) -> io::Result<()> {
            // 100ms のタイムアウトで非ブロッキングチェック
            if event::poll(Duration::from_millis(100))? {
                match event::read()? {
                    Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                        self.handle_key_event(key_event).await
                    }
                    _ => {}
                }
            }
            Ok(())
        }

    async fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            KeyCode::Left => {
                let _ = self.spotify_client.skip_track(SkipDirection::Previous).await;
            },
            KeyCode::Right => {
                let _ = self.spotify_client.skip_track(SkipDirection::Next).await;
            },
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

// ANCHOR: impl Widget
impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // 曲情報を取得
        let (track_name, artist_names, duration_ms) = self.spotify_client.spotify_player.item
            .as_ref()
            .map(|track| {
                let artists = track.artists.iter()
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                (track.name.as_str(), artists, track.duration_ms)
            })
            .unwrap_or(("No track playing", String::new(), 0));

        let progress_ms = self.spotify_client.spotify_player.progress_ms.unwrap_or(0);

        // プログレスの計算
        let progress_ratio = if duration_ms > 0 {
            (progress_ms as f64 / duration_ms as f64 * 100.0) as u16
        } else {
            0
        };

        // 時間表示の作成
        let time_display = format!("{} / {}", format_time(progress_ms), format_time(duration_ms));

        // レイアウトを作成
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // タイトル
                Constraint::Length(1), // 区切り線
                Constraint::Length(1), // 空行
                Constraint::Length(1), // 曲名
                Constraint::Length(1), // アーティスト名
                Constraint::Length(1), // 空行
                Constraint::Length(1), // 時間表示
                Constraint::Length(1), // プログレスバー
                Constraint::Min(0),    // 余白
            ])
            .split(area);

        // タイトルを表示
        let title = Line::from(" Now Playing ".bold());
        Paragraph::new(title)
            .centered()
            .render(chunks[0], buf);

        // 区切り線を表示
        let separator = "─".repeat(area.width as usize);
        let separator_line = Line::from(separator);
        Paragraph::new(separator_line)
            .render(chunks[1], buf);

        // 曲名を表示
        let track_line = Line::from(track_name.to_string().green().bold());
        Paragraph::new(track_line)
            .centered()
            .render(chunks[3], buf);

        // アーティスト名を表示
        let artist_line = Line::from(artist_names.to_string().green());
        Paragraph::new(artist_line)
            .centered()
            .render(chunks[4], buf);

        // 時間表示
        let time_line = Line::from(time_display.clone().cyan());
        Paragraph::new(time_line)
            .centered()
            .render(chunks[6], buf);

        // プログレスバーを表示（左右に余白を追加）
        let progress_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(2), // 左の余白
                Constraint::Min(0),    // プログレスバー
                Constraint::Length(2), // 右の余白
            ])
            .split(chunks[7]);

        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(Color::Green))
            .percent(progress_ratio)
            .label("");
        gauge.render(progress_chunks[1], buf);
    }
}


