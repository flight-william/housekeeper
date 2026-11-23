#![allow(clippy::all)]

#[allow(dead_code)]
fn helper() -> Result<(), String> {
    Ok(())
}

pub fn prod() {
    let _ = helper();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn slow() {
        let _ = helper();
        let _ = foo(bar(baz()));
    }

    #[test]
    #[ignore = "needs network"]
    fn net() {
        helper().unwrap();
    }
}

#[expect(clippy::needless_return)]
fn returns() -> i32 {
    return 1;
}

#[cfg_attr(test, allow(dead_code))]
fn maybe_dead() {}

#[cfg(test)]
mod more_tests {
    #[test]
    #[cfg_attr(not(feature = "net"), ignore)]
    fn needs_net() {}
}
