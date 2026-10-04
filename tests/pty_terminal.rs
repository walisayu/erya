use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::fs;
use std::io::{Read, Write};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

#[test]
fn terminal_session_handles_input_resize_save_quit_and_restoration() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!("erya-pty-{unique}"));
    let path = directory.join("互動測試.txt");
    fs::create_dir(&directory).expect("create PTY fixture directory");

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open native PTY");
    let mut command = CommandBuilder::new(erya_binary());
    command.arg(&path);
    let mut child = pair
        .slave
        .spawn_command(command)
        .expect("spawn Erya in PTY");
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().expect("clone PTY reader");
    let output_thread = thread::spawn(move || {
        let mut output = Vec::new();
        reader.read_to_end(&mut output).expect("read PTY output");
        output
    });
    let mut writer = pair.master.take_writer().expect("take PTY writer");
    answer_windows_cursor_position_query(&mut *writer);

    thread::sleep(Duration::from_millis(200));
    writer
        .write_all("甲乙🙂".as_bytes())
        .expect("type Unicode text");
    writer.flush().expect("flush typed text");
    thread::sleep(Duration::from_millis(100));
    pair.master
        .resize(PtySize {
            rows: 30,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("resize PTY");
    writer.write_all(b"\x18").expect("send Ctrl+X");
    writer.flush().expect("flush quit key");
    thread::sleep(Duration::from_millis(150));
    writer.write_all(b"s").expect("confirm save and quit");
    writer.flush().expect("flush save confirmation");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll Erya process") {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().expect("kill timed-out Erya process");
            timed_out = true;
            break child.wait().expect("wait for killed Erya process");
        }
        thread::sleep(Duration::from_millis(25));
    };
    drop(writer);
    drop(pair.master);
    let output = output_thread.join().expect("join PTY reader");
    let rendered = String::from_utf8_lossy(&output);

    assert!(
        !timed_out,
        "Erya did not exit after Ctrl+X; output: {rendered:?}"
    );

    assert!(status.success(), "Erya exited unsuccessfully: {status:?}");
    assert!(rendered.contains("爾雅 Erya"));
    assert!(
        rendered.contains("\u{1b}[?1049h"),
        "alternate screen was not entered"
    );
    assert!(
        rendered.contains("\u{1b}[?1049l"),
        "alternate screen was not left"
    );
    #[cfg(not(windows))]
    assert!(
        rendered.contains("\u{1b}[?7h"),
        "terminal line wrapping was not restored"
    );
    assert!(
        rendered.contains("\u{1b}[?2004l"),
        "bracketed paste was not disabled during restoration"
    );
    assert_eq!(
        fs::read_to_string(&path).expect("read PTY-saved document"),
        "甲乙🙂"
    );

    fs::remove_dir_all(directory).expect("remove PTY fixture directory");
}
