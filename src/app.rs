use crate::api::oauth::SpotifyOAuth;
use crate::api::spotify::{SpotifyClient, SkipDirection, Playlist};
use crate::utils::format_time;
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols::border,
    text::Line,
    widgets::{Block, Gauge, Paragraph, Widget, List, ListItem, ListState},
    DefaultTerminal, Frame,
};
use reqwest::Client;
use std::{io, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Page {
    PlaylistList,
    NowPlaying,
}

pub struct App {
    spotify_client: SpotifyClient,
    exit: bool,
    current_track_name: Option<String>,
    current_page: Page,
    playlists: Vec<Playlist>,
    playlist_state: ListState,
}

impl App {
    pub async fn new() -> Result<Self> {
        //1. get oauth
        let access_token = SpotifyOAuth::init()
            .await
            .map_err(|e| color_eyre::eyre::eyre!("{}", e))?;

        // init spotify_client and get current song
        let spotify_client = SpotifyClient::new(Client::new(), &access_token)
            .init()
            .await
            .map_err(|e| color_eyre::eyre::eyre!("{}", e))?;

        let current_track_name = spotify_client
            .spotify_player
            .item
            .as_ref()
            .map(|track| track.name.clone());

        // プレイリストを取得
        let playlists = spotify_client
            .get_user_playlists()
            .await
            .unwrap_or_default();

        let mut playlist_state = ListState::default();
        if !playlists.is_empty() {
            playlist_state.select(Some(0));
        }

        Ok(Self {
            spotify_client,
            exit: false,
            current_track_name,
            current_page: Page::PlaylistList,
            playlists,
            playlist_state,
        })
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
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
        match self.current_page {
            Page::PlaylistList => self.handle_playlist_list_key(key_event).await,
            Page::NowPlaying => self.handle_now_playing_key(key_event).await,
        }
    }

    async fn handle_playlist_list_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(selected) = self.playlist_state.selected() {
                    if selected > 0 {
                        self.playlist_state.select(Some(selected - 1));
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(selected) = self.playlist_state.selected() {
                    if selected < self.playlists.len() - 1 {
                        self.playlist_state.select(Some(selected + 1));
                    }
                }
            }
            KeyCode::Enter => {
                if let Some(selected) = self.playlist_state.selected() {
                    if let Some(playlist) = self.playlists.get(selected) {
                        // プレイリストを再生
                        let _ = self.spotify_client.play_playlist(&playlist.id).await;
                        // 再生画面に遷移
                        self.current_page = Page::NowPlaying;
                    }
                }
            }
            _ => {}
        }
    }

    async fn handle_now_playing_key(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            KeyCode::Esc | KeyCode::Char('p') => {
                // プレイリスト一覧に戻る
                self.current_page = Page::PlaylistList;
            }
            KeyCode::Left => {
                let _ = self
                    .spotify_client
                    .skip_track(SkipDirection::Previous)
                    .await;
            }
            KeyCode::Right => {
                let _ = self.spotify_client.skip_track(SkipDirection::Next).await;
            }
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
        // 背景を黒でクリア
        let background = Block::default().style(Style::default().bg(Color::Black));
        background.render(area, buf);

        match self.current_page {
            Page::PlaylistList => self.render_playlist_list(area, buf),
            Page::NowPlaying => self.render_now_playing(area, buf),
        }
    }
}

impl App {
    fn render_playlist_list(&mut self, area: Rect, buf: &mut Buffer) {
        let custom_green = Color::Rgb(0x0A, 0xE1, 0x64);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // ヘッダー
                Constraint::Min(0),    // プレイリストリスト
                Constraint::Length(2), // フッター
            ])
            .split(area);

        // ヘッダー
        let title = Line::from(" Your Playlists ".bold().fg(custom_green));
        let header_block = Block::bordered()
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(custom_green));
        let header = Paragraph::new(title)
            .centered()
            .block(header_block);
        header.render(layout[0], buf);

        // プレイリストリスト
        let items: Vec<ListItem> = self
            .playlists
            .iter()
            .map(|playlist| {
                let track_count = format!(" ({} tracks)", playlist.tracks.total);
                ListItem::new(format!("{}{}", playlist.name, track_count))
                    .style(Style::default().fg(Color::White))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::bordered()
                    .border_set(border::ROUNDED)
                    .border_style(Style::default().fg(custom_green))
            )
            .highlight_style(
                Style::default()
                    .bg(custom_green)
                    .fg(Color::Black)
                    .bold()
            )
            .highlight_symbol("> ");

        ratatui::widgets::StatefulWidget::render(list, layout[1], buf, &mut self.playlist_state);

        // フッター（操作ガイド）
        let help = Line::from(vec![
            "↑/k:Up ".fg(custom_green),
            "↓/j:Down ".fg(custom_green),
            "Enter:Play ".fg(custom_green),
            "q:Quit".fg(custom_green),
        ]);
        let footer = Paragraph::new(help).centered();
        footer.render(layout[2], buf);
    }

    fn render_now_playing(&self, area: Rect, buf: &mut Buffer) {
        // カスタムカラーを定義
        let custom_green = Color::Rgb(0x0A, 0xE1, 0x64);

        // 曲情報を取得
        let (track_name, artist_names, duration_ms) = self
            .spotify_client
            .spotify_player
            .item
            .as_ref()
            .map(|track| {
                let artists = track
                    .artists
                    .iter()
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
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // 上部の余白
                Constraint::Length(1), // タイトル 1
                Constraint::Length(1), // 区切り線 2
                Constraint::Length(3), // 空行 3
                Constraint::Length(1), // 曲名 4
                Constraint::Length(1), // アーティスト名
                Constraint::Length(2), // 空行
                Constraint::Length(3), // プログレスバー（枠込み）
                Constraint::Length(1), // 時間表示
                Constraint::Min(0),    // 余白
                Constraint::Length(2), // フッター
            ])
            .split(area);

        // タイトルを表示
        let title = Line::from(" Now Playing ".bold().fg(custom_green));
        Paragraph::new(title).centered().render(layout[1], buf);

        // 区切り線を表示
        let separator = "─".repeat(area.width as usize);
        let separator_line = Line::from(separator.fg(custom_green));
        Paragraph::new(separator_line).render(layout[2], buf);

        // 曲名を表示
        let track_line = Line::from(track_name.to_string().fg(custom_green).bold());
        Paragraph::new(track_line)
            .centered()
            .render(layout[4], buf);

        // アーティスト名を表示
        let artist_line = Line::from(artist_names.to_string().fg(custom_green));
        Paragraph::new(artist_line)
            .centered()
            .render(layout[5], buf);

        // プログレスバーのレイアウト
        let progress_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(2), // 左の余白
                Constraint::Min(0),    // プログレスバー
                Constraint::Length(2), // 右の余白
            ])
            .split(layout[7]);

        // プログレスバーに枠を追加
        let progress_block = Block::bordered()
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(custom_green));

        let progress_inner = progress_block.inner(progress_layout[1]);
        progress_block.render(progress_layout[1], buf);

        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(custom_green))
            .percent(progress_ratio)
            .label("");
        gauge.render(progress_inner, buf);

        // 時間表示のレイアウト
        let time_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(2), // 左の余白
                Constraint::Min(0),    // 中央エリア
                Constraint::Length(2), // 右の余白
            ])
            .split(layout[8]);

        // 中央エリアをさらに分割（再生時間とプログレスバーと残り時間の幅を揃える）
        let time_inner_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(8), // 再生時間
                Constraint::Min(0),    // 中央の余白
                Constraint::Length(8), // 残り時間
            ])
            .split(time_layout[1]);

        // 再生時間を表示
        let current_time_line = Line::from(current_time.fg(custom_green));
        Paragraph::new(current_time_line).render(time_inner_layout[0], buf);

        // 残り時間を表示
        let remaining_time_line = Line::from(remaining_time.fg(custom_green));
        Paragraph::new(remaining_time_line)
            .alignment(Alignment::Right)
            .render(time_inner_layout[2], buf);

        // フッター（操作ガイド）
        let help = Line::from(vec![
            "←:Prev ".fg(custom_green),
            "→:Next ".fg(custom_green),
            "p/Esc:Playlists ".fg(custom_green),
            "q:Quit".fg(custom_green),
        ]);
        let footer = Paragraph::new(help).centered();
        footer.render(layout[10], buf);
    }
}
