#!/usr/bin/env python3
"""Run a real PTY smoke test against the Kairox TUI binary.

The ratatui TestBackend cannot catch terminal integration regressions such as
startup text leaking into the alternate screen or raw-mode input not rendering.
This script starts agent-tui in a pseudo-terminal, types into the composer, and
asserts on bytes emitted to the terminal.
"""

from __future__ import annotations

import os
import pty
import re
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time
from pathlib import Path


ANSI_RE = re.compile(rb"\x1b\[[0-?]*[ -/]*[@-~]|\x1b[()][A-Za-z0-9]|\x1b[=>78]|\x1b\[\?[0-9;]*[hl]")


def strip_ansi(data: bytes) -> str:
    return ANSI_RE.sub(b"", data).decode("utf-8", errors="replace")


def set_pty_size(fd: int, rows: int, cols: int) -> None:
    winsize = struct.pack("HHHH", rows, cols, 0, 0)
    termios.tcsetwinsize(fd, (rows, cols))
    # tcsetwinsize is not consistently enough for child startup on every Unix.
    import fcntl

    fcntl.ioctl(fd, termios.TIOCSWINSZ, winsize)


def read_available(fd: int, timeout: float) -> bytes:
    chunks: list[bytes] = []
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        ready, _, _ = select.select([fd], [], [], 0.05)
        if not ready:
            continue
        try:
            chunk = os.read(fd, 65536)
        except OSError:
            break
        if not chunk:
            break
        chunks.append(chunk)
    return b"".join(chunks)


def wait_for(fd: int, buffer: bytearray, predicate, timeout: float, label: str) -> None:
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        buffer.extend(read_available(fd, 0.2))
        if predicate(bytes(buffer), strip_ansi(bytes(buffer))):
            return
    screen = strip_ansi(bytes(buffer))
    raise AssertionError(f"Timed out waiting for {label}.\nCaptured screen:\n{screen}")


def send_and_wait(
    fd: int,
    buffer: bytearray,
    data: bytes,
    predicate,
    timeout: float,
    label: str,
) -> None:
    buffer.clear()
    os.write(fd, data)
    wait_for(fd, buffer, predicate, timeout, label)


def shell_without_overlay(text: str) -> bool:
    overlay_titles = [
        "Command Palette",
        "Help / Keybindings",
        "Model Profile",
        "MCP Servers",
        "Skills Manager",
        "Plugin Manager",
    ]
    return (
        "Projects" in text
        and "Sessions" in text
        and "sessions:" in text
        and all(title not in text for title in overlay_titles)
    )


def composer_without_overlay(text: str) -> bool:
    overlay_titles = [
        "Command Palette",
        "Help / Keybindings",
        "Model Profile",
        "MCP Servers",
        "Skills Manager",
        "Plugin Manager",
    ]
    return ">" in text and all(title not in text for title in overlay_titles)


def tui_command(repo_root: Path) -> list[str]:
    explicit = os.environ.get("KAIROX_TUI_BIN")
    if explicit:
        return [explicit]
    return ["cargo", "run", "-p", "agent-tui", "--"]


def main() -> int:
    repo_root = Path(__file__).resolve().parents[1]
    master, slave = pty.openpty()
    set_pty_size(master, rows=30, cols=120)

    env = os.environ.copy()
    env["TERM"] = "xterm-256color"
    env["RUST_BACKTRACE"] = "0"
    real_home = Path(env.get("HOME", str(Path.home())))
    env.setdefault("CARGO_HOME", str(real_home / ".cargo"))
    env.setdefault("RUSTUP_HOME", str(real_home / ".rustup"))

    with tempfile.TemporaryDirectory(prefix="kairox-tui-smoke-home-") as home:
        env["HOME"] = home
        process = subprocess.Popen(
            tui_command(repo_root),
            cwd=repo_root,
            env=env,
            stdin=slave,
            stdout=slave,
            stderr=slave,
            start_new_session=True,
            close_fds=True,
        )
        os.close(slave)
        captured = bytearray()
        try:
            wait_for(
                master,
                captured,
                lambda raw, text: b"\x1b[?1049h" in raw
                and "Projects" in text
                and "Sessions" in text,
                20.0,
                "alternate-screen TUI shell",
            )

            raw_before_input = bytes(captured)
            text_before_input = strip_ansi(raw_before_input)
            forbidden = ["Kairox TUI", "Available model profiles", "Using profile:"]
            leaked = [value for value in forbidden if value in text_before_input]
            if leaked:
                raise AssertionError(
                    "Startup diagnostics leaked into the terminal screen: "
                    + ", ".join(leaked)
                    + "\nCaptured screen:\n"
                    + text_before_input
                )

            os.write(master, b"hello-pty")
            wait_for(
                master,
                captured,
                lambda _raw, text: "hello-pty" in text,
                5.0,
                "typed composer text",
            )
            rendered = strip_ansi(bytes(captured))
            if ">" not in rendered or "hello-pty" not in rendered:
                raise AssertionError(
                    "Composer prompt or typed text was not visible.\nCaptured screen:\n"
                    + rendered
                )

            send_and_wait(
                master,
                captured,
                b"\x7f" * len("hello-pty"),
                lambda _raw, text: "hello-pty" not in text,
                5.0,
                "composer text cleared",
            )

            send_and_wait(
                master,
                captured,
                b"\x1bOP",
                lambda _raw, text: "Help / Keybindings" in text and "Global shortcuts" in text,
                5.0,
                "F1 help overlay",
            )
            send_and_wait(
                master,
                captured,
                b"\x1b",
                lambda _raw, text: composer_without_overlay(text),
                5.0,
                "help overlay closed",
            )

            send_and_wait(
                master,
                captured,
                b"\x10",
                lambda _raw, text: "Command Palette" in text and ":compact" in text,
                5.0,
                "Ctrl+P command palette",
            )
            send_and_wait(
                master,
                captured,
                b"mcp",
                lambda _raw, text: "MCP: open manager" in text,
                5.0,
                "command palette filtering",
            )
            send_and_wait(
                master,
                captured,
                b"\x0d",
                lambda _raw, text: "MCP Servers" in text,
                10.0,
                "command palette opens MCP overlay",
            )
            send_and_wait(
                master,
                captured,
                b"\x1b",
                lambda _raw, text: composer_without_overlay(text),
                5.0,
                "MCP overlay closed",
            )

            overlay_shortcuts = [
                (b"\x0c", "Model Profile", "Ctrl+L model overlay"),
                (b"\x13", "Skills Manager", "Ctrl+S skills overlay"),
            ]
            for key_sequence, title, label in overlay_shortcuts:
                send_and_wait(
                    master,
                    captured,
                    key_sequence,
                    lambda _raw, text, marker=title: marker in text,
                    5.0,
                    label,
                )
                send_and_wait(
                    master,
                    captured,
                    b"\x1b",
                    lambda _raw, text: composer_without_overlay(text),
                    5.0,
                    f"{label} closed",
                )

            send_and_wait(
                master,
                captured,
                b"\x10",
                lambda _raw, text: "Command Palette" in text,
                5.0,
                "command palette re-opened",
            )
            send_and_wait(
                master,
                captured,
                b"plugins",
                lambda _raw, text: "Plugins: open manager" in text,
                5.0,
                "plugins command filtered",
            )
            send_and_wait(
                master,
                captured,
                b"\x0d",
                lambda _raw, text: "Plugin Manager" in text,
                10.0,
                "command palette opens plugins overlay",
            )
            send_and_wait(
                master,
                captured,
                b"\x1b",
                lambda _raw, text: composer_without_overlay(text),
                5.0,
                "plugins overlay closed",
            )
        finally:
            if process.poll() is None:
                try:
                    os.killpg(process.pid, signal.SIGTERM)
                except (PermissionError, ProcessLookupError):
                    process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except (PermissionError, ProcessLookupError):
                        process.kill()
                    process.wait(timeout=5)
            os.close(master)

    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except AssertionError as error:
        print(error, file=sys.stderr)
        raise SystemExit(1)
