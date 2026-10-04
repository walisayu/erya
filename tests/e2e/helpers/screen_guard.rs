use crate::known_codes::KNOWN_BACKEND_CODES;

const DIRTY_VALUES: &[&str] = &[
    "unexpected error",
    "internal server error",
    "undefined",
    "NaN",
    "[object Object]",
    "thread 'main' panicked",
    "stack backtrace",
];

pub fn assert_screen_healthy(screen_id: &str, output: &str, required: &[&str]) {
    assert!(
        output.contains("\u{1b}["),
        "{screen_id} did not emit terminal control output"
    );
    for text in required {
        assert!(
            output.contains(text),
            "{screen_id} is missing required text {text:?}; output: {output:?}"
        );
    }
    let lowercase = output.to_lowercase();
    for dirty in DIRTY_VALUES {
        assert!(
            !lowercase.contains(&dirty.to_lowercase()),
            "{screen_id} rendered dirty value {dirty:?}; output: {output:?}"
        );
    }
    for code in KNOWN_BACKEND_CODES {
        assert!(
            !output.contains(code),
            "{screen_id} leaked backend code {code:?}; output: {output:?}"
        );
    }
}
