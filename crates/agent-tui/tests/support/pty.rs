use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};

pub struct PtyHarness {
    reader_rx: mpsc::Receiver<Vec<u8>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    captured: Vec<u8>,
    home_dir: PathBuf,
    rows: usize,
    cols: usize,
}

impl PtyHarness {
    pub fn spawn(repo_root: &Path, command: Vec<String>, rows: u16, cols: u16) -> Self {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("PTY should open");

        let program = resolve_program(repo_root, &command[0]);
        let mut builder = CommandBuilder::new(program);
        builder.cwd(repo_root);
        for arg in &command[1..] {
            builder.arg(arg);
        }

        let real_home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let cargo_home =
            std::env::var("CARGO_HOME").unwrap_or_else(|_| format!("{real_home}/.cargo"));
        let rustup_home =
            std::env::var("RUSTUP_HOME").unwrap_or_else(|_| format!("{real_home}/.rustup"));
        let temp_home = std::env::temp_dir().join(format!(
            "kairox-tui-smoke-home-{}-{}",
            std::process::id(),
            monotonic_nonce()
        ));
        std::fs::create_dir_all(&temp_home).expect("temporary HOME should be created");

        builder.env("TERM", "xterm-256color");
        builder.env("RUST_BACKTRACE", "0");
        builder.env("HOME", &temp_home);
        builder.env("CARGO_HOME", cargo_home);
        builder.env("RUSTUP_HOME", rustup_home);

        let child = pair
            .slave
            .spawn_command(builder)
            .expect("TUI command should spawn in PTY");
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .expect("PTY reader should clone");
        let writer: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(
            pair.master.take_writer().expect("PTY writer should open"),
        ));
        let (reader_tx, reader_rx) = mpsc::channel();

        // The reader thread also auto-replies to DSR (Device Status Report)
        // queries (`ESC[6n`) that crossterm emits during startup to read the
        // cursor position. In a real terminal the emulator answers with
        // `ESC[row;colR`; in a headless CI PTY nobody replies, so crossterm
        // times out after 2 s and the TUI exits with an error. We intercept
        // the query here and write back a synthetic reply.
        let dsr_writer = Arc::clone(&writer);
        std::thread::spawn(move || {
            let mut buf = [0; 8192];
            // Small ring buffer to detect ESC[6n across read boundaries.
            let mut tail = Vec::<u8>::with_capacity(8);
            let dsr_sequence: &[u8] = b"\x1b[6n";
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let chunk = &buf[..n];

                        // Check for DSR across the boundary of the previous
                        // chunk and the start of the current one, then inside
                        // the current chunk itself.
                        tail.extend_from_slice(chunk);
                        let dsr_count = count_subsequence(&tail, dsr_sequence);
                        // Keep only the last few bytes for boundary detection.
                        if tail.len() > dsr_sequence.len() {
                            let keep_from = tail.len() - dsr_sequence.len() + 1;
                            tail.drain(..keep_from);
                        }

                        if dsr_count > 0 {
                            if let Ok(mut w) = dsr_writer.lock() {
                                for _ in 0..dsr_count {
                                    // Reply: cursor at row 1, col 1.
                                    let _ = w.write_all(b"\x1b[1;1R");
                                }
                                let _ = w.flush();
                            }
                        }

                        if reader_tx.send(chunk.to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
        });

        Self {
            reader_rx,
            writer,
            child,
            captured: Vec::new(),
            home_dir: temp_home,
            rows: rows as usize,
            cols: cols as usize,
        }
    }

    pub fn wait_for(
        &mut self,
        timeout: Duration,
        label: &str,
        predicate: impl Fn(&[u8], &str) -> bool,
    ) {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            self.drain_for(Duration::from_millis(50));
            let rendered = self.rendered_text();
            if predicate(&self.captured, &rendered) {
                return;
            }
            if let Ok(Some(status)) = self.child.try_wait() {
                panic!(
                    "TUI process exited while waiting for {label}: {status:?}\nCaptured screen:\n{}",
                    self.rendered_text()
                );
            }
        }

        panic!(
            "timed out waiting for {label}\nCaptured screen:\n{}\nRaw tail:\n{}",
            self.rendered_text(),
            escaped_tail(&self.captured)
        );
    }

    pub fn send_and_wait(
        &mut self,
        data: &[u8],
        timeout: Duration,
        label: &str,
        predicate: impl Fn(&[u8], &str) -> bool,
    ) {
        {
            let mut w = self.writer.lock().expect("PTY writer lock");
            w.write_all(data).expect("PTY input should be writable");
            w.flush().expect("PTY input should flush");
        }
        self.wait_for(timeout, label, predicate);
    }

    pub fn send(&mut self, data: &[u8]) {
        let mut w = self.writer.lock().expect("PTY writer lock");
        w.write_all(data).expect("PTY input should be writable");
        w.flush().expect("PTY input should flush");
    }

    pub fn visible_screen(&self) -> String {
        self.rendered_text()
    }

    fn rendered_text(&self) -> String {
        render_ansi_screen(&self.captured, self.rows, self.cols)
    }

    fn drain_for(&mut self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            match self.reader_rx.recv_timeout(Duration::from_millis(10)) {
                Ok(chunk) => self.captured.extend(chunk),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    }
}

impl Drop for PtyHarness {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.home_dir);
    }
}

pub fn strip_ansi(data: &[u8]) -> String {
    render_ansi_screen(data, 60, 200)
}

fn render_ansi_screen(data: &[u8], rows: usize, cols: usize) -> String {
    let rows = rows.max(1);
    let cols = cols.max(1);
    let mut screen = vec![vec![' '; cols]; rows];
    let mut row = 0usize;
    let mut col = 0usize;
    let mut saved_row = 0usize;
    let mut saved_col = 0usize;
    let rendered = String::from_utf8_lossy(data);
    let mut chars = rendered.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\x1b' => match chars.next() {
                Some('[') => {
                    let mut params = String::new();
                    let mut final_ch = None;
                    for next in chars.by_ref() {
                        if ('@'..='~').contains(&next) {
                            final_ch = Some(next);
                            break;
                        }
                        params.push(next);
                    }
                    apply_csi(
                        &params,
                        final_ch.unwrap_or_default(),
                        &mut screen,
                        &mut row,
                        &mut col,
                    );
                }
                Some('(' | ')') => {
                    let _ = chars.next();
                }
                Some('7') => {
                    saved_row = row;
                    saved_col = col;
                }
                Some('8') => {
                    row = saved_row.min(rows - 1);
                    col = saved_col.min(cols - 1);
                }
                Some('=' | '>') | None => {}
                Some(_) => {}
            },
            '\r' => col = 0,
            '\n' => {
                row = (row + 1).min(rows - 1);
                col = 0;
            }
            '\x08' => col = col.saturating_sub(1),
            ch if ch.is_control() => {}
            ch => {
                screen[row][col] = ch;
                col += 1;
                if col >= cols {
                    col = 0;
                    row = (row + 1).min(rows - 1);
                }
            }
        }
    }

    screen
        .into_iter()
        .map(|line| line.into_iter().collect::<String>().trim_end().to_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

fn apply_csi(
    params: &str,
    command: char,
    screen: &mut [Vec<char>],
    row: &mut usize,
    col: &mut usize,
) {
    let rows = screen.len();
    let cols = screen[0].len();
    let values = parse_csi_values(params);
    let first = values.first().copied().unwrap_or(0);

    match command {
        'H' | 'f' => {
            *row = values
                .first()
                .copied()
                .unwrap_or(1)
                .saturating_sub(1)
                .min(rows - 1);
            *col = values
                .get(1)
                .copied()
                .unwrap_or(1)
                .saturating_sub(1)
                .min(cols - 1);
        }
        'A' => *row = row.saturating_sub(first.max(1)),
        'B' => *row = (*row + first.max(1)).min(rows - 1),
        'C' => *col = (*col + first.max(1)).min(cols - 1),
        'D' => *col = col.saturating_sub(first.max(1)),
        'G' => *col = first.max(1).saturating_sub(1).min(cols - 1),
        'J' => clear_screen(screen, *row, *col, first),
        'K' => clear_line(screen, *row, *col, first),
        _ => {}
    }
}

fn parse_csi_values(params: &str) -> Vec<usize> {
    params
        .trim_start_matches('?')
        .split(';')
        .filter_map(|part| part.parse::<usize>().ok())
        .collect()
}

fn clear_screen(screen: &mut [Vec<char>], row: usize, col: usize, mode: usize) {
    let rows = screen.len();
    let cols = screen[0].len();
    match mode {
        1 => {
            for line in screen.iter_mut().take(row) {
                line.fill(' ');
            }
            screen[row][..=col.min(cols - 1)].fill(' ');
        }
        2 | 3 => {
            for line in screen {
                line.fill(' ');
            }
        }
        _ => {
            screen[row][col.min(cols - 1)..].fill(' ');
            for line in screen.iter_mut().take(rows).skip(row + 1) {
                line.fill(' ');
            }
        }
    }
}

fn clear_line(screen: &mut [Vec<char>], row: usize, col: usize, mode: usize) {
    let cols = screen[0].len();
    match mode {
        1 => screen[row][..=col.min(cols - 1)].fill(' '),
        2 => screen[row].fill(' '),
        _ => screen[row][col.min(cols - 1)..].fill(' '),
    }
}

fn escaped_tail(data: &[u8]) -> String {
    let start = data.len().saturating_sub(4000);
    data[start..]
        .iter()
        .flat_map(|byte| std::ascii::escape_default(*byte))
        .map(char::from)
        .collect()
}

pub fn tui_command() -> Vec<String> {
    if let Ok(explicit) = std::env::var("KAIROX_TUI_BIN") {
        return vec![explicit];
    }
    vec![env!("CARGO_BIN_EXE_agent-tui").into()]
}

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("agent-tui manifest should be under crates/agent-tui")
        .to_path_buf()
}

pub fn has_visible_text(screen: &str, expected: &str) -> bool {
    screen.contains(expected) || screen.replace(' ', "").contains(&expected.replace(' ', ""))
}

fn monotonic_nonce() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos()
}

fn resolve_program(repo_root: &Path, program: &str) -> PathBuf {
    let path = PathBuf::from(program);
    if path.is_absolute() {
        return path;
    }

    if path.components().count() > 1 {
        return repo_root.join(path);
    }

    path
}

/// Count non-overlapping occurrences of `needle` in `haystack`.
fn count_subsequence(haystack: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() || haystack.len() < needle.len() {
        return 0;
    }
    let mut count = 0;
    let mut start = 0;
    while start + needle.len() <= haystack.len() {
        if &haystack[start..start + needle.len()] == needle {
            count += 1;
            start += needle.len();
        } else {
            start += 1;
        }
    }
    count
}
