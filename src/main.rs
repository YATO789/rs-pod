mod api;
use api::oauth::SpotifyOAuth;
use color_eyre::Result;
use ratatui::{
    buffer::Buffer,
    layout::{Rect, Layout, Constraint, Direction},
    style::{Stylize, Style, Color},
    symbols::border,
    text::Line,
    widgets::{Block, Paragraph, Widget, Gauge},
    DefaultTerminal, Frame,
};
use crossterm::{event::{self, Event, KeyCode, KeyEvent, KeyEventKind}};
use std::{io, time::Duration};
use reqwest::Client;
use crate::api::spotify::{SpotifyClient, SkipDirection};
use ratatui_image::{picker::Picker, picker::ProtocolType, Image as RatatuiImage, Resize};
use image::DynamicImage;

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
    picker: Picker,
    album_art_image: Option<DynamicImage>,
    current_track_name: Option<String>,
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

        // Initialize picker with Halfblocks protocol
        // フォントサイズ: (幅, 高さ) 正方形表示のため 1:1 の比率
        let mut picker = Picker::new((10, 25));
        picker.protocol_type = ProtocolType::Halfblocks;

        // Download album art if available
        let mut album_art_image = None;
        let mut current_track_name = None;

        if let Some(track) = &spotify_client.spotify_player.item {
            current_track_name = Some(track.name.clone());
            if let Some(image) = track.album.images.first() {
                if let Ok(dyn_img) = spotify_client.download_image(&image.url).await {
                    album_art_image = Some(dyn_img);
                }
            }
        }

        Ok(Self{
            spotify_client,
            exit: false,
            picker,
            album_art_image,
            current_track_name,
        })
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
                    // Check if track changed
                    let new_track_name = player.item.as_ref().map(|t| t.name.clone());
                    if new_track_name != self.current_track_name {
                        self.current_track_name = new_track_name;
                        // Download new album art
                        if let Some(track) = &player.item {
                            if let Some(image) = track.album.images.first() {
                                if let Ok(dyn_img) = self.spotify_client.download_image(&image.url).await {
                                    self.album_art_image = Some(dyn_img);
                                }
                            }
                        } else {
                            self.album_art_image = None;
                        }
                    }
                    self.spotify_client.spotify_player = player;
                }
                last_update = tokio::time::Instant::now();
            }
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
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
impl Widget for &mut App {
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
        let current_time = format_time(progress_ms);
        let remaining_time = format!("-{}", format_time(duration_ms - progress_ms));

        // レイアウトを作成
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // タイトル
                Constraint::Length(1), // 区切り線
                Constraint::Length(1), // 空行
                Constraint::Length(20), // アルバムアート
                Constraint::Length(1), // 空行
                Constraint::Length(1), // 曲名
                Constraint::Length(1), // アーティスト名
                Constraint::Length(1), // 空行
                Constraint::Length(3), // プログレスバー（枠込み）
                Constraint::Length(1), // 時間表示
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

        // アルバムアートを表示
        if let Some(ref image) = self.album_art_image {
            let image_area = chunks[3];

            // 画像を中央に配置するために水平レイアウトを作成
            let horizontal_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(35), // 左の余白
                    Constraint::Percentage(30), // 画像エリア
                    Constraint::Percentage(35), // 右の余白
                ])
                .split(image_area);

            // 正方形のエリアを作成（ターミナルフォントのアスペクト比を考慮）
            let img_width = horizontal_chunks[1].width;
            let square_height = img_width; // halfblocksは1セルに2ピクセル分の高さを格納するため

            let vertical_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(square_height.min(horizontal_chunks[1].height)),
                    Constraint::Min(0),
                ])
                .split(horizontal_chunks[1]);

            // Halfblocksプロトコルを使用して画像を描画
            if let Ok(protocol) = self.picker.new_protocol(image.clone(), vertical_chunks[0], Resize::Fit(Some(ratatui_image::FilterType::Lanczos3))) {
                let image_widget = RatatuiImage::new(protocol.as_ref());
                image_widget.render(vertical_chunks[0], buf);
            }
        }

        // 曲名を表示
        let track_line = Line::from(track_name.to_string().green().bold());
        Paragraph::new(track_line)
            .centered()
            .render(chunks[5], buf);

        // アーティスト名を表示
        let artist_line = Line::from(artist_names.to_string().green());
        Paragraph::new(artist_line)
            .centered()
            .render(chunks[6], buf);

        // プログレスバーのレイアウト
        let progress_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(2),  // 左の余白
                Constraint::Min(0),     // プログレスバー
                Constraint::Length(2),  // 右の余白
            ])
            .split(chunks[8]);

        // プログレスバーに枠を追加
        let progress_block = Block::bordered()
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(Color::Green));

        let progress_inner = progress_block.inner(progress_chunks[1]);
        progress_block.render(progress_chunks[1], buf);

        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(Color::Green))
            .percent(progress_ratio)
            .label("");
        gauge.render(progress_inner, buf);

        // 時間表示のレイアウト
        let time_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(2),  // 左の余白
                Constraint::Min(0),     // 中央エリア
                Constraint::Length(2),  // 右の余白
            ])
            .split(chunks[9]);

        // 中央エリアをさらに分割（再生時間とプログレスバーと残り時間の幅を揃える）
        let time_inner_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(8),  // 再生時間
                Constraint::Min(0),     // 中央の余白
                Constraint::Length(8),  // 残り時間
            ])
            .split(time_chunks[1]);

        // 再生時間を表示
        let current_time_line = Line::from(current_time.cyan());
        Paragraph::new(current_time_line)
            .render(time_inner_chunks[0], buf);

        // 残り時間を表示
        let remaining_time_line = Line::from(remaining_time.cyan());
        Paragraph::new(remaining_time_line)
            .alignment(ratatui::layout::Alignment::Right)
            .render(time_inner_chunks[2], buf);
    }
}


