mod api;
use api::oauth::SpotifyOAuth;
use color_eyre::Result;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Paragraph, Widget},
    DefaultTerminal, Frame,
};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use std::io;
use reqwest::Client;

use crate::api::spotify::SpotifyClient;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let mut terminal = ratatui::init();
    let app_result = App::new().await?.run(&mut terminal).await;
    ratatui::restore();
    app_result
}


struct App{
    access_token : String,
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

        Ok(Self{access_token,spotify_client,exit : false})
    }

    async fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()>{
        while !self.exit {
             terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

        fn handle_events(&mut self) -> io::Result<()> {
            match event::read()? {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    self.handle_key_event(key_event)
                }
                _ => {}
            };
            Ok(())
        }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            KeyCode::Left => (),
            KeyCode::Right => (),
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
        let title = Line::from(" Counter App Tutorial ".bold());
        let instructions = Line::from(vec![
            " Decrement ".into(),
            "<Left>".blue().bold(),
            " Increment ".into(),
            "<Right>".blue().bold(),
            " Quit ".into(),
            "<Q> ".blue().bold(),
        ]);
        let block = Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.centered())
            .border_set(border::THICK);

        let track_name = self.spotify_client.spotify_player.item
            .as_ref()
            .map(|track| track.name.as_str())
            .unwrap_or("No track playing");

        let counter_text = Text::from(vec![Line::from(vec![
            "Now Playing: ".into(),
            track_name.to_string().yellow(),
        ])]);

        Paragraph::new(counter_text)
            .centered()
            .block(block)
            .render(area, buf);
    }
}

