mod app;
mod event;
mod tui;
mod model;
mod tmux;
mod adapter;
mod scripts;
mod ui;
mod config;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging to file (not stdout, since we own the terminal)
    tracing_subscriber::fmt()
        .with_writer(|| {
            let log_dir = dirs::data_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("agents-ui");
            std::fs::create_dir_all(&log_dir).ok();
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_dir.join("agents-ui.log"))
                .unwrap()
        })
        .with_ansi(false)
        .init();

    let mut terminal = tui::init()?;
    let mut app = app::App::new().await?;
    let result = app.run(&mut terminal).await;
    tui::restore()?;
    result
}
