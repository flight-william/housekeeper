#[cfg(test)]
mod tests {
use std::net::{TcpListener, TcpStream};
use std::process::Command;

#[test]
fn strings_and_comments_are_not_calls() {
    let note = "std::thread::sleep(d) Instant::now() env::var(\"HOME\") reqwest::get(u)";
    // std::thread::sleep(d); Instant::now(); std::env::var("HOME"); thread_rng()
    assert!(!note.is_empty());
}

#[test]
fn gives_a_child_a_temp_home() {
    let home = std::env::temp_dir().join("home");
    let mut cmd = Command::new("env");
    cmd.env("HOME", &home);
    let _path = std::env::var("PATH");
}

#[test]
fn seeded_generator() {
    let _rng = StdRng::seed_from_u64(4);
}

#[test]
fn fixed_zone() {
    let _t = chrono::Utc.timestamp_opt(0, 0);
    let _d = std::time::Duration::from_secs(1);
}

#[test]
fn connects_to_its_own_listener() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let _a = TcpStream::connect(listener.local_addr().unwrap());
    let addr = listener.local_addr().unwrap();
    let _b = TcpStream::connect(addr);
    let _c = TcpStream::connect("127.0.0.1:9");
    let _d = TcpStream::connect("localhost:9");
}

#[test]
fn mentions_a_path() {
    let _m = "files live under /Users/ on a Mac";
}

#[test]
fn asserts_on_the_stand_in_log() {
    let log = String::new();
    assert!(log.contains("find-generic-password"));
    assert_eq!(log, "add-generic-password -s svc");
    assert_ne!(
        log,
        "delete-generic-password -s svc"
    );
}
}
