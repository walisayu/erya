use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct PtyFixture {
    directory: PathBuf,
    path: PathBuf,
}

impl PtyFixture {
    pub fn new(filename: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let sequence = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let process = std::process::id();
        let directory =
            std::env::temp_dir().join(format!("erya-e2e-{process}-{unique}-{sequence}"));
        fs::create_dir(&directory).expect("create E2E fixture directory");
        let path = directory.join(filename);
        Self { directory, path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PtyFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}
