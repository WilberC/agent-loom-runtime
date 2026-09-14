use crate::state::State;
use anyhow::Result;
use crossterm::{
    cursor::Show,
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
};
use std::{io, sync::Arc, time::Duration};

pub fn run(state: Arc<State>) -> Result<()> {
    enable_raw_mode()?;
    let mut guard = TerminalGuard {
        terminal: None,
        stdout: Some(io::stdout()),
        raw_mode: true,
        alternate_screen: false,
    };
    execute!(
        guard.stdout.as_mut().expect("stdout is present"),
        EnterAlternateScreen
    )?;
    guard.alternate_screen = true;
    let backend = CrosstermBackend::new(guard.stdout.take().expect("stdout is present"));
    guard.terminal = Some(Terminal::new(backend)?);

    let result = loop_app(
        guard.terminal.as_mut().expect("terminal is initialized"),
        state,
    );
    let restore_result = guard.restore();
    match (result, restore_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error.into()),
    }
}

fn loop_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: Arc<State>,
) -> Result<()> {
    loop {
        let status = state.status()?;
        terminal.draw(|frame| render(frame, &status))?;
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    return Ok(());
                }
            }
        }
    }
}

pub fn render(frame: &mut Frame<'_>, status: &crate::state::Status) {
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
                .clone()
                .unwrap_or_else(|| "online status is reported by the daemon".to_owned()),
        )
        .block(Block::default().title("Last error").borders(Borders::ALL)),
        chunks[2],
    );
}

struct TerminalGuard {
    terminal: Option<Terminal<CrosstermBackend<io::Stdout>>>,
    stdout: Option<io::Stdout>,
    raw_mode: bool,
    alternate_screen: bool,
}

impl TerminalGuard {
    fn restore(&mut self) -> io::Result<()> {
        let mut result = Ok(());
        if self.alternate_screen {
            result = if let Some(terminal) = self.terminal.as_mut() {
                execute!(terminal.backend_mut(), LeaveAlternateScreen)
            } else if let Some(stdout) = self.stdout.as_mut() {
                execute!(stdout, LeaveAlternateScreen)
            } else {
                Err(io::Error::other("terminal output is unavailable"))
            };
            self.alternate_screen = false;
        }
        if self.raw_mode {
            if let Err(error) = disable_raw_mode() {
                if result.is_ok() {
                    result = Err(error);
                }
            }
            self.raw_mode = false;
        }
        let cursor_result = if let Some(terminal) = self.terminal.as_mut() {
            terminal.show_cursor()
        } else if let Some(stdout) = self.stdout.as_mut() {
            execute!(stdout, Show)
        } else {
            Ok(())
        };
        if let Err(error) = cursor_result {
            if result.is_ok() {
                result = Err(error);
            }
        }
        result
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
