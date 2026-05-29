//! Real PTY smoke coverage for terminal integration.
//!
//! The Ratatui `TestBackend` covers deterministic rendering, but it cannot
//! catch raw-mode input, alternate-screen, or terminal byte regressions. This
//! ignored test is run explicitly by CI's TUI build job.

use std::time::Duration;

mod support;

use support::pty::{has_visible_text, repo_root, tui_command, PtyHarness};

const WAIT_SHORT: Duration = Duration::from_secs(5);
const WAIT_STARTUP: Duration = Duration::from_secs(20);
const WAIT_OVERLAY: Duration = Duration::from_secs(10);

#[test]
#[ignore = "spawns the compiled TUI in a real PTY; CI invokes it explicitly"]
fn real_pty_smoke_covers_terminal_startup_input_and_overlays() {
    let mut terminal = PtyHarness::spawn(&repo_root(), tui_command(), 30, 120);

    terminal.wait_for(WAIT_STARTUP, "alternate-screen TUI shell", |raw, text| {
        raw.windows(b"\x1b[?1049h".len())
            .any(|window| window == b"\x1b[?1049h")
            && text.contains("Projects")
            && text.contains("Sessions")
    });

    let startup_screen = terminal.visible_screen();
    for forbidden in ["Kairox TUI", "Available model profiles", "Using profile:"] {
        assert!(
            !startup_screen.contains(forbidden),
            "startup diagnostics leaked into the terminal screen: {forbidden}\n{startup_screen}"
        );
    }

    terminal.send(b"hello-rust-pty");
    terminal.wait_for(WAIT_SHORT, "typed composer text", |_raw, text| {
        text.contains("hello-rust-pty")
    });
    let typed_screen = terminal.visible_screen();
    assert!(
        typed_screen.contains('>') && typed_screen.contains("hello-rust-pty"),
        "composer prompt or typed text was not visible:\n{typed_screen}"
    );

    terminal.send_and_wait(
        &vec![b'\x7f'; "hello-rust-pty".len()],
        WAIT_SHORT,
        "composer text cleared",
        |_raw, text| !text.contains("hello-rust-pty"),
    );

    terminal.send_and_wait(b"\x1bOP", WAIT_SHORT, "F1 help overlay", |_raw, text| {
        text.contains("Help / Keybindings") && text.contains("Global shortcuts")
    });
    terminal.send_and_wait(b"\x1b", WAIT_SHORT, "help overlay closed", |_raw, text| {
        composer_without_overlay(text)
    });

    open_palette_entry(
        &mut terminal,
        b"mcp",
        "MCP: open manager",
        "command palette opens MCP overlay",
        |_raw, text| text.contains("MCP Servers"),
    );
    terminal.send_and_wait(b"\x1b", WAIT_SHORT, "MCP overlay closed", |_raw, text| {
        composer_without_overlay(text)
    });

    for (key_sequence, title, label) in [
        (b"\x0c".as_slice(), "Model Profile", "Ctrl+L model overlay"),
        (
            b"\x13".as_slice(),
            "Skills Manager",
            "Ctrl+S skills overlay",
        ),
    ] {
        terminal.send_and_wait(key_sequence, WAIT_SHORT, label, |_raw, text| {
            text.contains(title)
        });
        terminal.send_and_wait(
            b"\x1b",
            WAIT_SHORT,
            &format!("{label} closed"),
            |_raw, text| composer_without_overlay(text),
        );
    }

    for (filter_text, expected_entry, title, label) in [
        (
            b"plugins".as_slice(),
            "Plugins: open manager",
            "Plugin Manager",
            "plugins overlay",
        ),
        (
            b"agents".as_slice(),
            "agents",
            "Agent Settings",
            "agents overlay",
        ),
        (
            b"hooks".as_slice(),
            "hooks",
            "Hooks Settings",
            "hooks overlay",
        ),
        (
            b"instructions".as_slice(),
            "instructions",
            "Instructions",
            "instructions overlay",
        ),
    ] {
        open_palette_entry(
            &mut terminal,
            filter_text,
            expected_entry,
            &format!("command palette opens {label}"),
            |_raw, text| text.contains(title),
        );
        terminal.send_and_wait(
            b"\x1b",
            WAIT_SHORT,
            &format!("{label} closed"),
            |_raw, text| composer_without_overlay(text),
        );
    }

    for (filter_text, expected_entry, prefill) in [
        (
            b"project create".as_slice(),
            "Create a new local project",
            ":project create ",
        ),
        (
            b"project import".as_slice(),
            "existing project path",
            ":project import ",
        ),
        (
            b"project worktree".as_slice(),
            "worktree",
            ":project worktree ",
        ),
    ] {
        open_palette_entry(
            &mut terminal,
            filter_text,
            expected_entry,
            &format!("command palette prefills {}", prefill.trim()),
            |_raw, text| has_visible_text(text, prefill),
        );
        terminal.send_and_wait(
            &vec![b'\x7f'; prefill.len()],
            WAIT_SHORT,
            &format!("{} prefill cleared", prefill.trim()),
            |_raw, text| !has_visible_text(text, prefill),
        );
    }

    terminal.send_and_wait(
        b"\x1bt",
        WAIT_SHORT,
        "trace sidebar visible",
        |_raw, text| {
            has_visible_text(text, "Trace | Tasks | Memory")
                || has_visible_text(text, "[Trace] | Tasks | Memory")
        },
    );
    terminal.send(b"\x1b3");
    terminal.wait_for(WAIT_SHORT, "trace panel focused", |_raw, text| {
        has_visible_text(text, "Trace | Tasks | Memory")
            || has_visible_text(text, "[Trace] | Tasks | Memory")
    });
    terminal.send(b"\x1b[C");
    terminal.wait_for(WAIT_SHORT, "tasks tab reached", |_raw, text| {
        text.contains("[Tasks]") && has_visible_text(text, "No tasks yet")
    });
    terminal.send(b"\x1b[C");
    terminal.wait_for(WAIT_SHORT, "memory tab reached", |_raw, text| {
        text.contains("[Memory]")
    });
    terminal.send(b"s");
    terminal.wait_for(WAIT_SHORT, "memory scope cycles", |_raw, text| {
        text.contains("scope:ses")
    });
}

fn open_palette_entry(
    terminal: &mut PtyHarness,
    filter_text: &[u8],
    expected_entry: &str,
    label: &str,
    result_predicate: impl Fn(&[u8], &str) -> bool,
) {
    terminal.send_and_wait(
        b"\x10",
        WAIT_SHORT,
        &format!("{label} palette opened"),
        |_raw, text| text.contains("Command Palette"),
    );
    terminal.send_and_wait(
        filter_text,
        WAIT_SHORT,
        &format!("{label} command filtered"),
        |_raw, text| has_visible_text(text, expected_entry),
    );
    terminal.send_and_wait(b"\x0d", WAIT_OVERLAY, label, result_predicate);
}

fn composer_without_overlay(text: &str) -> bool {
    let overlay_titles = [
        "Command Palette",
        "Help / Keybindings",
        "Model Profile",
        "MCP Servers",
        "Skills Manager",
        "Plugin Manager",
        "Agent Settings",
        "Hooks Settings",
        "Instructions",
        "Archive Manager",
    ];

    text.contains('>') && overlay_titles.iter().all(|title| !text.contains(title))
}
