use crate::pty_session::PtySession;

pub const CTRL_O: &[u8] = b"\x0f";
pub const CTRL_P: &[u8] = b"\x10";
pub const CTRL_S: &[u8] = b"\x13";
pub const CTRL_W: &[u8] = b"\x17";
pub const CTRL_X: &[u8] = b"\x18";
pub const CTRL_Y: &[u8] = b"\x19";
pub const CTRL_Z: &[u8] = b"\x1a";
pub const ESCAPE: &[u8] = b"\x1b";
pub const F1: &[u8] = b"\x1bOP";
pub const F2: &[u8] = b"\x1bOQ";
pub const F4: &[u8] = b"\x1bOS";
pub const F11: &[u8] = b"\x1b[23~";
pub const DOWN: &[u8] = b"\x1b[B";
pub const SHIFT_RIGHT: &[u8] = b"\x1b[1;2C";
pub const ENTER: &[u8] = b"\r";
pub const Q: &[u8] = b"q";
pub const N: &[u8] = b"n";
pub const Y: &[u8] = b"y";

pub fn enter_help(session: &mut PtySession) -> String {
    session.clear_output();
    session.send(F1);
    session.wait_for_text("按鍵說明")
}

pub fn leave_help(session: &mut PtySession) -> String {
    session.clear_output();
    session.send(F1);
    session.wait_for_text("Ctrl+S")
}

pub fn enter_file_browser(session: &mut PtySession) -> String {
    session.clear_output();
    session.send(CTRL_O);
    session.wait_for_text("開啟檔案")
}

pub fn leave_file_browser(session: &mut PtySession) -> String {
    session.clear_output();
    session.send(ESCAPE);
    session.wait_for_text("Ctrl+S")
}

pub fn cancel_file_browser_with_q(session: &mut PtySession) -> String {
    session.clear_output();
    session.send(Q);
    session.wait_for_text("Ctrl+S")
}

pub fn leave_overlay(session: &mut PtySession) -> String {
    session.clear_output();
    session.send(ESCAPE);
    session.wait_for_text("Ctrl+S")
}

pub fn enter_hex_view(session: &mut PtySession) -> String {
    session.clear_output();
    session.send(CTRL_P);
    session.wait_for_text("十六進位")
}

pub fn leave_hex_view(session: &mut PtySession) -> String {
    session.clear_output();
    session.send(CTRL_P);
    session.wait_for_text("Ctrl+S")
}

pub fn enter_search_prompt(session: &mut PtySession) -> String {
    session.clear_output();
    session.send(CTRL_W);
    session.wait_for_text("搜尋")
}

pub fn enter_settings(session: &mut PtySession) -> String {
    session.clear_output();
    session.send(F2);
    session.wait_for_text("系統設定")
}

pub fn enter_keymap_editor(session: &mut PtySession) -> String {
    session.clear_output();
    session.send(b"e");
    session.wait_for_text("按鍵配置編輯器")
}

pub fn leave_keymap_editor(session: &mut PtySession) -> String {
    session.clear_output();
    session.send(ESCAPE);
    session.wait_for_text("系統設定")
}

pub fn enter_comparison_with_second_file(session: &mut PtySession) -> String {
    session.clear_output();
    session.send(F4);
    session.wait_for_text("選擇左側檔案");
    session.clear_output();
    session.send(DOWN);
    session.send(ENTER);
    session.wait_for_text("選擇右側檔案");
    session.clear_output();
    session.send(DOWN);
    session.send(ENTER);
    session.wait_for_text("檔案比對")
}
