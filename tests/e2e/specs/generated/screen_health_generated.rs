use crate::editor_screen;
use crate::pty_fixture::PtyFixture;
use crate::pty_session::PtySession;
use crate::screen_guard::assert_screen_healthy;
use crate::screen_manifest::{SCREENS, ScreenRisk};
use std::fs;

#[test]
fn generated_screen_health_scans_every_executable_terminal_screen() {
    let executable = SCREENS.iter().filter(|screen| screen.executable).count();
    assert_eq!(executable, 12, "manifest executable count changed");
    assert_eq!(
        SCREENS
            .iter()
            .filter(|screen| screen.risk == ScreenRisk::P0)
            .count(),
        4,
        "manifest P0 count changed"
    );
    for screen in SCREENS {
        assert!(screen.screen_id.starts_with("SCREEN-TUI-"));
        assert!(!screen.name.is_empty());
        assert!(!screen.entry_action.is_empty());
        assert!(!screen.ready_text.is_empty());
        assert_eq!(screen.blocked_reason.is_some(), !screen.executable);
        if screen.critical {
            assert_eq!(screen.risk, ScreenRisk::P0);
        }
    }

    let mut session = PtySession::spawn(None, 24, 80);
    let editing = session.wait_for_text("爾雅 Erya");
    assert_screen_healthy("SCREEN-TUI-001", &editing, &["爾雅 Erya", "Ctrl+S"]);

    let prompt = editor_screen::enter_search_prompt(&mut session);
    assert_screen_healthy(
        "SCREEN-TUI-002",
        &prompt,
        &["搜尋", "Enter 確認", "Esc 取消"],
    );
    let editing = editor_screen::leave_overlay(&mut session);
    assert_screen_healthy("SCREEN-TUI-001", &editing, &["Ctrl+S"]);

    let mut close_session = PtySession::spawn(None, 24, 80);
    close_session.wait_for_text("爾雅 Erya");
    close_session.send("修改".as_bytes());
    close_session.clear_output();
    close_session.send(editor_screen::F11);
    let close_confirmation = close_session.wait_for_text("頁籤尚未儲存");
    assert_screen_healthy("SCREEN-TUI-004", &close_confirmation, &["頁籤尚未儲存"]);
    close_session.send(editor_screen::Y);
    close_session.wait_for_text("Ctrl+S");
    close_session.send(editor_screen::CTRL_X);
    assert!(close_session.finish().success);

    let browser = editor_screen::enter_file_browser(&mut session);
    assert_screen_healthy(
        "SCREEN-TUI-006",
        &browser,
        &["開啟檔案", "📂", "←/Backspace 上層", "Esc/Q 取消"],
    );
    editor_screen::cancel_file_browser_with_q(&mut session);
    editor_screen::enter_file_browser(&mut session);
    editor_screen::leave_file_browser(&mut session);

    let help = editor_screen::enter_help(&mut session);
    assert_screen_healthy("SCREEN-TUI-007", &help, &["按鍵說明"]);
    editor_screen::leave_help(&mut session);

    let settings = editor_screen::enter_settings(&mut session);
    assert_screen_healthy(
        "SCREEN-TUI-010",
        &settings,
        &["系統設定", "按鍵配置", "介面語言"],
    );
    let keymap_editor = editor_screen::enter_keymap_editor(&mut session);
    assert_screen_healthy(
        "SCREEN-TUI-011",
        &keymap_editor,
        &["按鍵配置編輯器", "另存新檔"],
    );
    editor_screen::leave_keymap_editor(&mut session);
    editor_screen::leave_overlay(&mut session);

    let comparison_fixture = PtyFixture::new("compare-left.txt");
    fs::write(
        comparison_fixture.path(),
        "相同行\n左側版本：100\n共同中段\n左側結尾\n共同結尾",
    )
    .expect("write left comparison fixture");
    let right_path = comparison_fixture
        .path()
        .parent()
        .expect("comparison fixture parent")
        .join("compare-right.txt");
    fs::write(
        &right_path,
        "相同行\n右側版本：120\n共同中段\n右側結尾\n共同結尾",
    )
    .expect("write right comparison fixture");
    let mut comparison_session = PtySession::spawn(Some(comparison_fixture.path()), 24, 100);
    comparison_session.wait_for_text("爾雅 Erya");
    let comparison = editor_screen::enter_comparison_with_second_file(&mut comparison_session);
    assert_screen_healthy(
        "SCREEN-TUI-012",
        &comparison,
        &[
            "檔案比對",
            "compare-left.txt",
            "compare-right.txt",
            "圖例",
            "▶",
            "N 下一個差異",
            "Shift+→",
        ],
    );
    comparison_session.clear_output();
    comparison_session.send(editor_screen::DOWN);
    comparison_session.send(editor_screen::DOWN);
    comparison_session.send(editor_screen::DOWN);
    comparison_session.wait_for_text("目前差異 1/2");
    comparison_session.clear_output();
    comparison_session.send(b"n");
    comparison_session.wait_for_text("目前差異 2/2");
    comparison_session.clear_output();
    comparison_session.send(b"p");
    comparison_session.wait_for_text("目前差異 1/2");
    comparison_session.send(editor_screen::SHIFT_RIGHT);
    comparison_session.wait_for_text("右側暫存檔");
    assert_eq!(
        fs::read_to_string(&right_path).expect("read unchanged right comparison fixture"),
        "相同行\n右側版本：120\n共同中段\n右側結尾\n共同結尾"
    );
    comparison_session.send(editor_screen::CTRL_S);
    comparison_session.wait_for_text("確定以暫存比對結果覆蓋原始檔案");
    comparison_session.send(editor_screen::N);
    comparison_session.wait_for_text("已取消覆蓋");
    comparison_session.send(editor_screen::ESCAPE);
    comparison_session.wait_for_text("暫存變更尚未套用");
    comparison_session.send(editor_screen::N);
    comparison_session.wait_for_text("Ctrl+S");
    assert_eq!(
        fs::read_to_string(&right_path).expect("read discarded right comparison fixture"),
        "相同行\n右側版本：120\n共同中段\n右側結尾\n共同結尾"
    );
    comparison_session.send(editor_screen::CTRL_X);
    assert!(comparison_session.finish().success);

    let hex_fixture = PtyFixture::new("hex-view.txt");
    fs::write(hex_fixture.path(), "Erya 十六進位\n").expect("write hex fixture");
    let mut hex_session = PtySession::spawn(Some(hex_fixture.path()), 24, 80);
    hex_session.wait_for_text("爾雅 Erya");
    let hex_view = editor_screen::enter_hex_view(&mut hex_session);
    assert_screen_healthy(
        "SCREEN-TUI-013",
        &hex_view,
        &[
            "W 每列 16/32",
            "00000000  45 72 79 61",
            "|Erya ",
            "位移 0x0 / 0x12",
            "每列 16 位元組",
        ],
    );
    hex_session.clear_output();
    hex_session.send(b"o");
    let decimal_offsets = hex_session.wait_for_text("位移欄改用 10 進位");
    assert_screen_healthy(
        "SCREEN-TUI-013",
        &decimal_offsets,
        &["       0  45 72 79 61", "      16  8D 0A"],
    );
    hex_session.clear_output();
    hex_session.send(b"g");
    hex_session.wait_for_text("跳至位移");
    hex_session.send(b"17");
    hex_session.send(editor_screen::ENTER);
    hex_session.wait_for_text("已跳至位移 17");
    editor_screen::leave_hex_view(&mut hex_session);
    hex_session.send(editor_screen::CTRL_X);
    assert!(hex_session.finish().success);

    let binary_fixture = PtyFixture::new("bytes.bin");
    fs::write(binary_fixture.path(), [0x00, 0x01, 0x41, 0x42, 0xFF, 0x0A])
        .expect("write binary fixture");
    let mut binary_session = PtySession::spawn(Some(binary_fixture.path()), 24, 80);
    let binary_view = binary_session.wait_for_text("二進位");
    assert_screen_healthy(
        "SCREEN-TUI-014",
        &binary_view,
        &[
            "0-9A-F 位元組  I 覆寫/插入",
            "F11 關閉",
            "00000000  00 01 41 42 FF 0A",
            "位移 0x0 / 0x6",
            "覆寫",
        ],
    );
    binary_session.clear_output();
    binary_session.send(b"41");
    let edited = binary_session.wait_for_text("00000000  41 01 41 42 FF 0A");
    assert_screen_healthy("SCREEN-TUI-014", &edited, &["|A.AB..", "位移 0x1 / 0x6"]);
    binary_session.send(editor_screen::CTRL_S);
    binary_session.wait_for_text("已儲存");
    assert_eq!(
        fs::read(binary_fixture.path()).expect("read saved binary fixture"),
        [0x41, 0x01, 0x41, 0x42, 0xFF, 0x0A]
    );
    binary_session.send(editor_screen::CTRL_X);
    assert!(binary_session.finish().success);

    session.clear_output();
    session.resize(5, 20);
    let small = session.wait_for_text("終端視窗太小");
    assert_screen_healthy("SCREEN-TUI-008", &small, &["終端視窗太小"]);

    session.clear_output();
    session.resize(24, 80);
    session.wait_for_text("Ctrl+S");
    session.send(editor_screen::CTRL_X);
    let result = session.finish();
    assert!(
        result.success,
        "clean PTY session failed: {:?}",
        result.output
    );
}
