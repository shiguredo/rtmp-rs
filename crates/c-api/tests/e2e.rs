use std::path::PathBuf;
use std::process::Command;

#[test]
fn test_simple_connection_example() {
    let project_root = get_project_root();
    let lib_path = project_root.join("target/debug/librtmp.a");
    ensure_static_library_exists(&project_root, &lib_path);

    let c_file = project_root.join("crates/c-api/tests/simple_connection.c");
    let output_path = project_root.join("target/debug/simple_connection");

    let status = Command::new("cc")
        .arg(&c_file)
        .arg("-o")
        .arg(&output_path)
        .arg(&lib_path)
        .arg("-I")
        .arg(project_root.join("crates/c-api/include"))
        .status()
        .expect("failed to compile simple_connection.c");
    assert!(
        status.success(),
        "compilation failed for simple_connection.c"
    );

    let status = Command::new(&output_path)
        .status()
        .expect("failed to execute simple_connection");
    assert!(status.success(), "simple_connection execution failed");
}

fn ensure_static_library_exists(project_root: &std::path::Path, lib_path: &std::path::Path) {
    if lib_path.exists() {
        return;
    }

    let status = Command::new("cargo")
        .arg("build")
        .arg("-p")
        .arg("c-api")
        .current_dir(project_root)
        .status()
        .expect("failed to build c-api static library");
    assert!(status.success(), "cargo build -p c-api failed");
    assert!(
        lib_path.exists(),
        "librtmp.a not found at {} after build",
        lib_path.display()
    );
}

fn get_project_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .expect("failed to find project root")
        .to_path_buf()
}
