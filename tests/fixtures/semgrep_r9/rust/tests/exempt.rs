// Cleanup outside a Drop impl or a teardown: each result has nobody to go to.
#[test]
fn cleans_up_inline() {
    let mut child = std::process::Command::new("true").spawn().unwrap();
    let _ = child.kill();
    let _ = child.wait();
    let _ = child.try_wait();
    let _ = std::fs::remove_file("x");
    let _ = std::fs::remove_dir_all("d");
    let _ = fs::remove_file(path);
    let _ = remove_dir_all(&dir);
    let (tx, _rx) = std::sync::mpsc::channel();
    let _ = tx.send(1);
    let _ = std::io::stdout().flush();
    let _ = stream.shutdown(std::net::Shutdown::Both);
    let _ = handle.join();
    let _ = self.0
        .kill();
    // Arguments nested two levels deep: still a send.
    let _ = tx.send(run_child(&os(&words), Duration::from_millis(300)));
}
