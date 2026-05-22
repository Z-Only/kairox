use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};

const ANSI_ESC: u8 = 0x1b;

pub struct PtyHarness {
    reader_rx: mpsc::Receiver<Vec<u8>>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    captured: Vec<u8>,
    home_dir: PathBuf,
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
        let writer = pair.master.take_writer().expect("PTY writer should open");
        let (reader_tx, reader_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut buf = [0; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if reader_tx.send(buf[..n].to_vec()).is_err() {
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
            let rendered = strip_ansi(&self.captured);
            if predicate(&self.captured, &rendered) {
                return;
            }
            if let Ok(Some(status)) = self.child.try_wait() {
                panic!(
                    "TUI process exited while waiting for {label}: {status:?}\nCaptured screen:\n{}",
                    strip_ansi(&self.captured)
                );
            }
        }

        panic!(
            "timed out waiting for {label}\nCaptured screen:\n{}",
            strip_ansi(&self.captured)
        );
    }

    pub fn send_and_wait(
        &mut self,
        data: &[u8],
        timeout: Duration,
        label: &str,
        predicate: impl Fn(&[u8], &str) -> bool,
    ) {
        self.captured.clear();
        self.writer
            .write_all(data)
            .expect("PTY input should be writable");
        self.writer.flush().expect("PTY input should flush");
        self.wait_for(timeout, label, predicate);
    }

    pub fn send(&mut self, data: &[u8]) {
        self.writer
            .write_all(data)
            .expect("PTY input should be writable");
        self.writer.flush().expect("PTY input should flush");
    }

    pub fn visible_screen(&self) -> String {
        strip_ansi(&self.captured)
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
    let mut stripped = Vec::with_capacity(data.len());
    let mut index = 0;
    while index < data.len() {
        if data[index] != ANSI_ESC {
            stripped.push(data[index]);
            index += 1;
            continue;
        }

        index += 1;
        if index >= data.len() {
            break;
        }

        match data[index] {
            b'[' => {
                index += 1;
                while index < data.len() && !matches!(data[index], b'@'..=b'~') {
                    index += 1;
                }
                index += usize::from(index < data.len());
            }
            b'(' | b')' => {
                index = (index + 2).min(data.len());
            }
            b'=' | b'>' | b'7' | b'8' => {
                index += 1;
            }
            _ => {}
        }
    }

    String::from_utf8_lossy(&stripped).into_owned()
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
