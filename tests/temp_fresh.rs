//! A test's temp folder is new and empty, never one an earlier run left. Folders are
//! kept on purpose and named by process id, which comes round again: a run that took
//! an old folder found the earlier run's commits and index in it.
//!
//! The only test in this file, so its first `fresh` takes the counter's first value.

#[path = "common/temp.rs"]
mod common;

#[test]
fn a_folder_an_earlier_run_left_under_this_process_id_is_never_reused() {
    // What an earlier run with this process id left: its first folder, work in it.
    let name = format!("house-reuse-{}-0", std::process::id());
    let left = std::env::temp_dir().join(name);
    std::fs::create_dir_all(&left).unwrap();
    std::fs::write(left.join("staged.txt"), "an earlier run's\n").unwrap();
    let left = left.canonicalize().unwrap();
    let dir = common::fresh("reuse");
    assert_ne!(dir, left, "an earlier run's folder was reused");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "{dir:?}");
    assert_ne!(common::fresh("reuse"), dir);
}
