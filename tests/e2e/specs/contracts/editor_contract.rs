use crate::editor_screen;
use crate::pty_session::PtySession;
use crate::screen_contract::assert_contract;

#[test]
fn screen_tui_001_editor_contract_exposes_current_primary_actions() {
    let mut session = PtySession::spawn(None, 24, 100);
    let output = session.wait_for_text("Ctrl+X");
    assert_contract(
        "SCREEN-TUI-001",
        &output,
        &[
            "爾雅 Erya",
            "F1",
            "Ctrl+S",
            "Ctrl+W",
            "Ctrl+X",
            "UTF-8",
            "LF",
        ],
    );
    session.send(editor_screen::CTRL_X);
    assert!(session.finish().success);
}
