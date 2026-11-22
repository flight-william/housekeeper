#[test]
fn drops_result() {
    let _ = std::fs::read_to_string("missing");
    // A cleanup call inside an ordinary one: the outer result is dropped.
    let _ = check(child.wait());
    // An ordinary call on the result of a cleanup call.
    let _ = tx.send(1).map(drop);
    let _ = killer();
}
