use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const CONDITION_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

fn answer_windows_cursor_position_query(writer: &mut dyn Write) {
    #[cfg(windows)]
    {
        // Headless ConPTY has no terminal frontend to answer its startup DSR.
        writer
            .write_all(b"\x1b[1;1R")
            .expect("answer ConPTY cursor position query");
        writer.flush().expect("flush ConPTY cursor position");
    }
    #[cfg(not(windows))]
    let _ = writer;
}

fn erya_binary() -> String {
    std::env::var("ERYA_TEST_BINARY").unwrap_or_else(|_| env!("CARGO_BIN_EXE_erya").to_owned())
}

pub struct SessionResult {
    pub success: bool,
    pub output: String,
}

pub struct PtySession {
    child: Option<Box<dyn Child + Send + Sync>>,
    master: Option<Box<dyn MasterPty + Send>>,
    writer: Option<Box<dyn Write + Send>>,
    output: Arc<Mutex<Vec<u8>>>,
    reader_thread: Option<JoinHandle<()>>,
}

impl PtySession {
    pub fn spawn(path: Option<&Path>, rows: u16, cols: u16) -> Self {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("open native PTY");
        let mut command = CommandBuilder::new(erya_binary());
        command.env("ERYA_LANG", "zh-TW");
        command.env("TERM", "xterm-256color");
        if let Some(path) = path {
            command.arg(path);
        }
        let child = pair
            .slave
            .spawn_command(command)
            .expect("spawn Erya in PTY");
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().expect("clone PTY reader");
        let mut writer = pair.master.take_writer().expect("take PTY writer");
        answer_windows_cursor_position_query(&mut *writer);
        let output = Arc::new(Mutex::new(Vec::new()));
        let reader_output = Arc::clone(&output);
        let reader_thread = thread::spawn(move || {
            let mut chunk = [0_u8; 4096];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(read) => reader_output
                        .lock()
                        .expect("lock PTY output")
                        .extend_from_slice(&chunk[..read]),
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
        });

        Self {
            child: Some(child),
            master: Some(pair.master),
            writer: Some(writer),
            output,
            reader_thread: Some(reader_thread),
        }
    }

    pub fn send(&mut self, bytes: &[u8]) {
        let writer = self.writer.as_mut().expect("PTY writer remains available");
        writer.write_all(bytes).expect("write PTY input");
        writer.flush().expect("flush PTY input");
    }

    pub fn resize(&self, rows: u16, cols: u16) {
        self.master
            .as_ref()
            .expect("PTY master remains available")
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("resize PTY");
    }

    pub fn clear_output(&self) {
        self.output.lock().expect("lock PTY output").clear();
    }

    pub fn output(&self) -> String {
        String::from_utf8_lossy(&self.output.lock().expect("lock PTY output")).into_owned()
    }

    pub fn wait_for_text(&self, expected: &str) -> String {
        let deadline = Instant::now() + CONDITION_TIMEOUT;
        loop {
            let output = self.output();
            if output.contains(expected) {
                return output;
            }
            assert!(
                Instant::now() < deadline,
                "PTY did not render {expected:?}; output: {output:?}"
            );
            thread::sleep(POLL_INTERVAL);
        }
    }

    pub fn finish(mut self) -> SessionResult {
        let deadline = Instant::now() + CONDITION_TIMEOUT;
        let status = loop {
            if let Some(status) = self
                .child
                .as_mut()
                .expect("PTY child remains available")
                .try_wait()
                .expect("poll Erya process")
            {
                break status;
            }
            if Instant::now() >= deadline {
                self.child
                    .as_mut()
                    .expect("PTY child remains available")
                    .kill()
                    .expect("kill timed-out Erya process");
                panic!("Erya did not exit; output: {:?}", self.output());
            }
            thread::sleep(POLL_INTERVAL);
        };

        self.writer.take();
        self.master.take();
        if let Some(reader_thread) = self.reader_thread.take() {
            reader_thread.join().expect("join PTY reader");
        }
        SessionResult {
            success: status.success(),
            output: self.output(),
        }
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        self.writer.take();
        self.master.take();
        if let Some(reader_thread) = self.reader_thread.take() {
            let _ = reader_thread.join();
        }
    }
}
