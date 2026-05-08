fn main() {
    inject_build_info();

    // Feature-gate which capability files tauri-build discovers. The pilot
    // capability is only included when BOTH the cargo feature `pilot` is
    // enabled (so the plugin crate is linked) AND the build is a debug
    // profile (mirroring the `#[cfg(all(debug_assertions, feature = "pilot"))]`
    // gate on plugin registration in `src/lib.rs`). Without these gates,
    // `tauri-build` would fail with "Permission pilot:default not found"
    // (when the crate isn't linked) or, in a release+feature build, succeed
    // at compile time but leave the plugin unregistered at runtime. We rely
    // on the `CARGO_FEATURE_PILOT` env var that cargo sets in build scripts
    // when the corresponding feature is enabled.
    let pilot_capability_enabled =
        std::env::var_os("CARGO_FEATURE_PILOT").is_some() && cfg!(debug_assertions);
    let attributes = if pilot_capability_enabled {
        // Default discovery: includes both default.json and pilot.json.
        tauri_build::Attributes::new()
    } else {
        // Restrict to default.json only.
        tauri_build::Attributes::new().capabilities_path_pattern("./capabilities/default.json")
    };

    if let Err(error) = tauri_build::try_build(attributes) {
        panic!("failed to run tauri-build: {error:#}");
    }
}

fn inject_build_info() {
    let version = env!("CARGO_PKG_VERSION").to_string();
    println!("cargo:rustc-env=KAIROX_VERSION={}", version);

    let git_hash = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=KAIROX_GIT_HASH={}", git_hash);

    let build_time = std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=KAIROX_BUILD_TIME={}", build_time);

    println!("cargo:rerun-if-env-changed=KAIROX_VERSION");
}
