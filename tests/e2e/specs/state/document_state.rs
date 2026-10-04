use crate::auth_state::assert_saved_document;
use crate::editor_screen;
use crate::pty_fixture::PtyFixture;
use crate::pty_session::PtySession;
use crate::screen_guard::assert_screen_healthy;
use crate::state_transitions::SC_EDIT_SAVE;

#[test]
fn sc_tui_001_undo_redo_save_preserves_durable_document_state() {
    let fixture = PtyFixture::new("狀態連續性.txt");
    let mut session = PtySession::spawn(Some(fixture.path()), 24, 80);
    session.wait_for_text("爾雅 Erya");
    session.clear_output();
    session.send("甲乙👩‍💻".as_bytes());
    session.send(editor_screen::CTRL_Z);
    session.send(editor_screen::CTRL_Y);
    session.send(editor_screen::CTRL_X);
    let confirmation = session.wait_for_text("文件尚未儲存");
    assert_screen_healthy("SCREEN-TUI-003", &confirmation, &["文件尚未儲存"]);
    session.send(b"s");
    let result = session.finish();

    assert!(result.success, "{SC_EDIT_SAVE} exited unsuccessfully");
    assert_saved_document(fixture.path(), "甲乙👩‍💻");
}
