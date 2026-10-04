use std::fs;
use std::path::Path;

pub fn assert_saved_document(path: &Path, expected: &str) {
    assert_eq!(
        fs::read_to_string(path).expect("read saved document side effect"),
        expected,
        "saved document is the durable state evidence"
    );
}
