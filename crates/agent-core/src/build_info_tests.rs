use super::*;

#[test]
fn from_env_never_panics() {
    // In test builds the KAIROX_* vars are not set, so we get fallbacks.
    let info = BuildInfo::from_env();
    // version falls back to CARGO_PKG_VERSION which is the workspace version
    assert!(!info.version.is_empty());
    assert_eq!(info.git_hash, "dev");
    assert_eq!(info.build_time, "unknown");
}

#[test]
fn display_format() {
    let info = BuildInfo {
        version: "0.11.0",
        git_hash: "abc1234",
        build_time: "2026-01-01T00:00:00Z",
    };
    assert_eq!(info.to_string(), "0.11.0 (abc1234 2026-01-01T00:00:00Z)");
}
