//! A fresh empty temp folder; for test files that need nothing else, and for the
//! crate's own unit tests (`src/lib.rs` includes this file as `temp`).

use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A new empty folder under the temp dir; left behind on purpose. Each is made with
/// `create_dir`, which fails if the folder exists: a process id comes round again, and
/// a folder an earlier run left under it is passed over for the next counter value,
/// never reused.
pub fn fresh(name: &str) -> PathBuf {
    let base = std::env::temp_dir();
    std::fs::create_dir_all(&base).unwrap();
    loop {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = base.join(format!("house-{name}-{}-{n}", std::process::id()));
        match std::fs::create_dir(&dir) {
            Ok(()) => return dir.canonicalize().unwrap(),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("cannot make {}: {e}", dir.display()),
        }
    }
}
