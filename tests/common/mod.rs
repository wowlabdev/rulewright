//! Shared process-level fixture helpers.

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

pub(crate) fn command(current_directory: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rulewright"));

    command.current_dir(current_directory);

    command
}

pub(crate) fn run(current_directory: &Path, arguments: &[&str]) -> Output {
    command(current_directory)
        .args(arguments)
        .output()
        .expect("Rulewright should run")
}

pub(crate) fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

pub(crate) fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("fixture parent directory should be created");
    }

    fs::write(path, contents).expect("fixture file should be written");
}

pub(crate) fn root_package(root: &Path, name: &str, source: &str) {
    write(
        &root.join("Cargo.toml"),
        &format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\nrust-version = \"1.85\"\n"
        ),
    );
    write(&root.join("src/lib.rs"), source);
}

pub(crate) fn initialize(root: &Path) {
    let root_argument = root.to_string_lossy();
    let output = run(
        root,
        &["--workspace-root", &root_argument, "--init", "--quiet"],
    );

    assert!(output.status.success(), "{}", text(&output));
}
