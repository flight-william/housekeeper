use std::time::SystemTime;

#[test]
fn sleeps() {
    std::thread::sleep(std::time::Duration::from_millis(1));
    let s = "std::thread::sleep(d) SystemTime::now()"; // std::thread::sleep(d)
    assert!(!s.is_empty());
}

#[test]
fn clock() {
    let now = SystemTime::now();
    assert!(now.elapsed().is_ok());
}
