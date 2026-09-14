use crate::state::State;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
};
use std::{io, sync::Arc, time::Duration};
pub fn run(state: Arc<State>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = loop {
        let status = state.status()?;
        terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(3),
                ])
                .split(frame.area());
            frame.render_widget(
                Paragraph::new("Agent Loom Runtime — q to quit")
                    .block(Block::default().borders(Borders::ALL)),
                chunks[0],
            );
            frame.render_widget(
                Paragraph::new(format!(
                    "enrolled: {} | commands: {} | queued events: {}",
                    status.identity_present, status.known_commands, status.queued_events
                ))
                .block(Block::default().borders(Borders::ALL)),
                chunks[1],
            );
            frame.render_widget(
                Paragraph::new(
                    status
                        .last_error
                        .unwrap_or_else(|| "online status is reported by the daemon".to_owned()),
                )
                .block(Block::default().title("Last error").borders(Borders::ALL)),
                chunks[2],
            );
        })?;
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break Ok(());
                }
            }
        }
    };
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}
