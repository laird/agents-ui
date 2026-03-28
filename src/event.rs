use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind};
use futures::StreamExt;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::model::status::AgentStatus;

/// All events the app processes.
#[derive(Debug)]
#[allow(dead_code)]
pub enum Event {
    /// Keyboard input
    Key(KeyEvent),
    /// Periodic tick for UI refresh
    Tick,
    /// Updated pane content from tmux
    PaneOutput {
        agent_id: String,
        content: String,
    },
    /// Agent status file changed
    StatusChange {
        agent_id: String,
        status: AgentStatus,
    },
    /// A swarm was discovered (on startup reconnect)
    SwarmDiscovered {
        session_name: String,
        repo_path: String,
    },
    /// Error from a background task
    Error(String),
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<Event>,
    tx: mpsc::UnboundedSender<Event>,
}

impl EventHandler {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let event_tx = tx.clone();

        // Spawn crossterm event reader
        tokio::spawn(async move {
            let mut reader = event::EventStream::new();
            loop {
                let crossterm_event = reader.next().await;
                match crossterm_event {
                    Some(Ok(CrosstermEvent::Key(key))) => {
                        // Only handle key press events, not release/repeat
                        if key.kind == KeyEventKind::Press {
                            if event_tx.send(Event::Key(key)).is_err() {
                                break;
                            }
                        }
                    }
                    Some(Err(_)) | None => break,
                    _ => {}
                }
            }
        });

        // Spawn tick timer
        let tick_tx = tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(250));
            loop {
                interval.tick().await;
                if tick_tx.send(Event::Tick).is_err() {
                    break;
                }
            }
        });

        Self { rx, tx }
    }

    /// Get a clone of the sender for background tasks to emit events.
    pub fn tx(&self) -> mpsc::UnboundedSender<Event> {
        self.tx.clone()
    }

    /// Receive the next event.
    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
}
