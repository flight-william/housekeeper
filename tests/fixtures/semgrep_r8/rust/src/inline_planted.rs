#[cfg(test)]
mod tests {
use std::process::Command;
use std::time::{Instant, SystemTime};

#[test]
fn sleeps() {
    std::thread::sleep(std::time::Duration::from_millis(1));
    thread::sleep(d);
    tokio::time::sleep(d);
}

#[test]
fn reads_the_clock() {
    let _a = std::time::SystemTime::now();
    let _b = SystemTime::now();
    let _c = std::time::Instant::now();
    let _d = Instant::now();
    let _e = chrono::Utc::now();
    let _f = Utc::now();
    let _g = chrono::Local::now();
    let _h = Local::now();
}

#[test]
fn draws_random() {
    let _a = rand::thread_rng();
    let _b = thread_rng();
    let _c = rand::rng();
    let _d: u8 = rand::random();
    let _e = rand::random::<u8>();
    let _f = StdRng::from_entropy();
    let _g = StdRng::from_os_rng();
}

#[test]
fn reads_the_zone() {
    let _a = chrono::Local.timestamp_opt(0, 0);
    let _b = Local.from_utc_datetime(&naive);
    let _c = utc.with_timezone(&chrono::Local);
    let _d = utc.with_timezone(&Local);
    let _e = iana_time_zone::get_timezone();
    let _f = sys_locale::get_locale();
}

#[test]
fn reads_home() {
    let _a = std::env::var("HOME");
    let _b = env::var("HOME");
    let _c = var("HOME");
    let _d = std::env::var_os("HOME");
    let _e = env::var_os("HOME");
    let _f = var_os("HOME");
    let _g = std::env::home_dir();
    let _h = env::home_dir();
    let _i = home::home_dir();
    let _j = dirs::home_dir();
}

#[test]
fn reads_the_keychain() {
    let _a = Command::new("security");
    let _b = Command::new("/usr/bin/security");
    let _c = std::process::Command::new("security");
    let _d = std::process::Command::new("/usr/bin/security");
    let _e = keyring::Entry::new("svc", "user");
    let _f = "find-generic-password";
}

#[test]
fn opens_the_network() {
    let _a = std::net::TcpStream::connect("example.invalid:80");
    let _b = TcpStream::connect(addr);
    let _c = std::net::TcpStream::connect_timeout(&addr, d);
    let _d = TcpStream::connect_timeout(&addr, d);
    let _e = reqwest::get("http://example.invalid");
    let _f = reqwest::blocking::get("http://example.invalid");
    let _g = reqwest::Client::new();
    let _h = reqwest::blocking::Client::new();
    let _i = ureq::get("http://example.invalid");
}

#[test]
fn names_machine_paths() {
    let _a = std::fs::read_to_string("/Users/someone/project/notes.txt");
    let _b = std::fs::File::open("/home/someone/project");
    let _c = std::path::Path::new("~/project").exists();
}
}
