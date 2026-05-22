#[test]
fn skills_overlay_uses_split_module_boundaries() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let module_dir = manifest_dir.join("src/components/skills_overlay");

    for file_name in ["mod.rs", "state.rs", "render.rs", "editor.rs", "tests.rs"] {
        assert!(
            module_dir.join(file_name).is_file(),
            "missing skills overlay module boundary: {file_name}"
        );
    }

    assert!(
        !manifest_dir
            .join("src/components/skills_overlay.rs")
            .exists(),
        "skills overlay should stay split across module boundary files"
    );
}
