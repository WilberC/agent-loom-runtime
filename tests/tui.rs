use agent_loom_runtime::{state::Status, tui};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};

fn rendered_buffer(width: u16, height: u16, status: Status) -> Buffer {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| tui::render(frame, &status)).unwrap();
    terminal.backend().buffer().clone()
}

fn contains_text(buffer: &Buffer, text: &str) -> bool {
    let lines = buffer
        .content
        .chunks(usize::from(buffer.area.width))
        .map(|cells| {
            cells
                .iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
        });
    lines.collect::<Vec<_>>().join("\n").contains(text)
}

#[test]
fn renders_runtime_status_deterministically() {
    let status = Status {
        identity_present: true,
        queued_events: 2,
        known_commands: 7,
        last_error: Some("connection refused".into()),
    };

    let first = rendered_buffer(100, 12, status.clone());
    let second = rendered_buffer(100, 12, status);

    assert_eq!(first, second);
    assert!(contains_text(&first, "Agent Loom Runtime"));
    assert!(contains_text(
        &first,
        "enrolled: true | commands: 7 | queued events: 2"
    ));
    assert!(contains_text(&first, "connection refused"));
}

#[test]
fn renders_without_panicking_at_minimum_supported_size() {
    let status = Status {
        identity_present: false,
        queued_events: 0,
        known_commands: 0,
        last_error: None,
    };

    let buffer = rendered_buffer(1, 1, status);

    assert_eq!(buffer.area.width, 1);
    assert_eq!(buffer.area.height, 1);
}

#[test]
fn renders_default_diagnostic_when_no_error_exists() {
    let status = Status {
        identity_present: false,
        queued_events: 0,
        known_commands: 0,
        last_error: None,
    };

    let buffer = rendered_buffer(100, 12, status);

    assert!(contains_text(
        &buffer,
        "online status is reported by the daemon"
    ));
}
