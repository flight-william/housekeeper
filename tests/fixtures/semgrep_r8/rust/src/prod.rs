use std::time::Instant;

pub fn wait() -> Option<String> {
    std::thread::sleep(std::time::Duration::from_millis(1));
    let _start = Instant::now();
    let _path = "/Users/someone/project";
    std::env::var("HOME").ok()
}

mod helpers {
    pub fn stamp() -> std::time::Instant {
        std::time::Instant::now()
    }
}
