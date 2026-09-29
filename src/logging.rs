use std::fs::OpenOptions;
use std::sync::{Mutex, Once};

static INIT: Once = Once::new();

/// Initializes a file-backed `tracing` subscriber so diagnostics never reach
/// the worksheet. Safe to call multiple times; only the first call takes effect.
pub fn init() {
    INIT.call_once(|| {
        let path = std::env::temp_dir().join("cvxx.log");
        if let Ok(file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = tracing_subscriber::fmt()
                .with_writer(Mutex::new(file))
                .with_ansi(false)
                .try_init();
        }
    });
}
