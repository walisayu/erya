use crate::auth_state::assert_saved_document;
use crate::editor_screen;
use crate::pty_fixture::PtyFixture;
use crate::pty_session::PtySession;
use crate::screen_guard::assert_screen_healthy;
use crate::state_transitions::SC_OVERLAY_RETURN;

#[test]
fn sc_tui_002_zero_shortcut_edit_help_return_save_and_restore_terminal() {
    let fixture = PtyFixture::new("黃金旅程.yaml");
    let mut session = PtySession::spawn(Some(fixture.path()), 24, 80);
    let editing = session.wait_for_text("爾雅 Erya");
    assert_screen_healthy("SCREEN-TUI-001", &editing, &["Ctrl+S"]);
    assert!(
        editing.contains("\u{1b}[?1049h"),
        "alternate screen was not entered"
    );

    let help = editor_screen::enter_help(&mut session);
    assert_screen_healthy("SCREEN-TUI-007", &help, &["按鍵說明"]);
    editor_screen::leave_help(&mut session);

    session.send(b"name: erya");
    session.send(editor_screen::CTRL_X);
    let confirmation = session.wait_for_text("文件尚未儲存");
    assert_screen_healthy("SCREEN-TUI-003", &confirmation, &["文件尚未儲存"]);
    session.send(b"s");
    let result = session.finish();

    assert!(result.success, "{SC_OVERLAY_RETURN} exited unsuccessfully");
    assert_saved_document(fixture.path(), "name: erya");
    assert!(
        result.output.contains("\u{1b}[?1049l"),
        "alternate screen was not left"
    );
    #[cfg(not(windows))]
    assert!(
        result.output.contains("\u{1b}[?7h"),
        "terminal wrapping was not restored"
    );
    assert!(
        result.output.contains("\u{1b}[?2004l"),
        "bracketed paste was not disabled"
    );
}
