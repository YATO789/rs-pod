mod api;
mod app;
mod utils;

use app::App;
use color_eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let mut terminal = ratatui::init();
    terminal.clear()?; // 初回だけクリア
    let app_result = App::new().await?.run(&mut terminal).await;
    ratatui::restore();
    app_result
}
