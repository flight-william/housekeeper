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
    }

    #[test]
    #[ignore = "needs network"]
    fn net() {
        helper().unwrap();
    }
}
