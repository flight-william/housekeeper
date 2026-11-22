#[test]
fn drops_result() {
    let _ = std::fs::read_to_string("missing");
}
