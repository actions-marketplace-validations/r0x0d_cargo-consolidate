use std::fs;
use std::process::Command;

use tempfile::TempDir;

fn cargo_consolidate_path() -> std::path::PathBuf {
    // Use the binary built by cargo test
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // remove test binary name
    path.pop(); // remove deps/
    path.push("cargo-consolidate");
    path
}

fn create_workspace(dir: &std::path::Path, members: &[(&str, &str)], workspace_toml: &str) {
    fs::write(dir.join("Cargo.toml"), workspace_toml).unwrap();

    for (name, content) in members {
        let member_dir = dir.join(name);
        fs::create_dir_all(&member_dir).unwrap();
        fs::write(member_dir.join("Cargo.toml"), content).unwrap();

        // Create minimal lib.rs so cargo doesn't complain
        let src_dir = member_dir.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("lib.rs"), "").unwrap();
    }
}

#[test]
fn test_clean_workspace_exits_zero() {
    let dir = TempDir::new().unwrap();

    let ws = r#"
[workspace]
members = ["a", "b"]
"#;
    let a = r#"
[package]
name = "a"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
"#;
    let b = r#"
[package]
name = "b"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
"#;

    create_workspace(dir.path(), &[("a", a), ("b", b)], ws);

    let output = Command::new(cargo_consolidate_path())
        .args([
            "consolidate",
            "--path",
            dir.path().to_str().unwrap(),
            "--no-color",
        ])
        .output()
        .expect("failed to run cargo-consolidate");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "Expected exit 0, got: {stdout}");
    assert!(stdout.contains("Version mismatches: 0"));
    assert!(stdout.contains("Shared dependencies: 1"));
}

#[test]
fn test_mismatch_workspace_exits_one() {
    let dir = TempDir::new().unwrap();

    let ws = r#"
[workspace]
members = ["a", "b"]
"#;
    let a = r#"
[package]
name = "a"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = "1.35"
"#;
    let b = r#"
[package]
name = "b"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = "1.37"
"#;

    create_workspace(dir.path(), &[("a", a), ("b", b)], ws);

    let output = Command::new(cargo_consolidate_path())
        .args([
            "consolidate",
            "--path",
            dir.path().to_str().unwrap(),
            "--no-color",
        ])
        .output()
        .expect("failed to run cargo-consolidate");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(1),
        "Expected exit 1, got: {stdout}"
    );
    assert!(stdout.contains("Version mismatches: 1"));
    assert!(stdout.contains("Version Mismatches"));
}

#[test]
fn test_mismatches_only_flag() {
    let dir = TempDir::new().unwrap();

    let ws = r#"
[workspace]
members = ["a", "b"]
"#;
    let a = r#"
[package]
name = "a"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = "1.35"
serde = "1.0"
"#;
    let b = r#"
[package]
name = "b"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = "1.37"
serde = "1.0"
"#;

    create_workspace(dir.path(), &[("a", a), ("b", b)], ws);

    let output = Command::new(cargo_consolidate_path())
        .args([
            "consolidate",
            "--path",
            dir.path().to_str().unwrap(),
            "--no-color",
            "--mismatches-only",
        ])
        .output()
        .expect("failed to run cargo-consolidate");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Version Mismatches"));
    assert!(!stdout.contains("Shared Dependencies"));
}
