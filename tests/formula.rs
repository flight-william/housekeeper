//! Formula/house.rb installs from the public repo, not from a path on one machine.

const FORMULA: &str = include_str!("../Formula/house.rb");
const REPO: &str = "https://github.com/flight-william/housekeeper";

#[test]
fn the_formula_installs_the_tagged_release_from_the_public_repo() {
    assert!(
        FORMULA.contains(&format!("  url \"{REPO}.git\",\n")),
        "{FORMULA}"
    );
    let version = env!("CARGO_PKG_VERSION");
    assert_eq!(version, "0.4.0");
    assert!(
        FORMULA.contains(&format!("      tag: \"v{version}\"\n")),
        "{FORMULA}"
    );
    assert!(
        FORMULA.contains(&format!("  version \"{version}\"\n")),
        "{FORMULA}"
    );
    assert!(
        FORMULA.contains(&format!("  head \"{REPO}.git\", branch: \"main\"\n")),
        "{FORMULA}"
    );
    assert!(
        FORMULA.contains(&format!("  homepage \"{REPO}\"\n")),
        "{FORMULA}"
    );
}

#[test]
fn the_formula_names_no_local_source() {
    assert!(!FORMULA.contains("file://"), "{FORMULA}");
    // No quoted absolute path: a source or build path on one machine.
    assert!(!FORMULA.contains("\"/"), "{FORMULA}");
}

#[test]
fn the_formula_installs_every_folder_the_binary_reads_at_run_time() {
    // The Semgrep adapter reads patterns/ and the R3 adapter reads scripts/;
    // after a Homebrew build the source tree is gone, so both must be
    // installed and pointed to by the wrapper.
    assert!(
        FORMULA.contains("pkgshare.install \"patterns\", \"scripts\""),
        "{FORMULA}"
    );
    assert!(
        FORMULA.contains("HOUSE_PATTERNS_DIR: pkgshare/\"patterns\""),
        "{FORMULA}"
    );
    assert!(
        FORMULA.contains("HOUSE_SCRIPTS_DIR:  pkgshare/\"scripts\""),
        "{FORMULA}"
    );
}
