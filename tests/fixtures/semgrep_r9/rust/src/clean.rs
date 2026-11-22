pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

// A fixture text a test feeds to a parser, not an attribute:
// #[allow(dead_code)] and #[ignore] here are words in a comment.
pub const SAMPLE: &str = "#[allow(dead_code)] fn x() {}";
pub const RAW: &str = r#"
#[allow(dead_code)]
#[ignore]
#[expect(unused)]
fn y() {}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    struct TempFile(std::path::PathBuf);

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn teardown(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn stops_its_child() {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_dir_all("d");
    }

    #[test]
    fn adds() {
        assert_eq!(add(1, 2), 3);
        let _file = TempFile(std::env::temp_dir().join("x"));
        teardown(std::path::Path::new("x"));
    }
}
