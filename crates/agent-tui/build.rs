fn main() {
    inject_build_info();
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
