use std::{fs, path::Path, process::Command};

fn run(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_custom-rulewright"))
        .args(arguments)
        .output()
        .expect("custom Rulewright wrapper should run")
}

fn text(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn write_project(root: &Path, source: &str) {
    fs::create_dir(root.join("src")).expect("fixture source directory should be created");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\nrust-version = \"1.85\"\n",
    )
    .expect("fixture manifest should be written");
    fs::write(root.join("src/lib.rs"), source).expect("fixture source should be written");
}

#[test]
fn downstream_rule_participates_in_the_complete_cli() {
    let list = run(&["--list", "--quiet"]);
    assert!(list.status.success());
    assert!(text(&list).contains("custom_no_placeholder"));

    let detail = run(&["--detail", "custom_no_placeholder", "--quiet"]);
    assert!(detail.status.success());
    assert!(text(&detail).contains("application-specific placeholder"));

    let temporary = tempfile::tempdir().expect("temporary project should be created");
    let root = temporary.path();

    write_project(root, "pub fn value() { /* CUSTOM_PLACEHOLDER */ }\n");
    let root_arg = root.to_string_lossy();
    let init = run(&["--workspace-root", &root_arg, "--init", "--quiet"]);

    assert!(init.status.success());
    let config_path = root.join("rulewright.toml");
    let config = fs::read_to_string(&config_path).expect("generated config should be readable");
    assert!(config.contains("[rules.custom_no_placeholder]"));

    let config_arg = config_path.to_string_lossy();
    let parsed = run(&["--parse-config", &config_arg, "--quiet"]);
    assert!(parsed.status.success());
    assert!(text(&parsed).contains("custom_no_placeholder"));

    let llm = run(&[
        "--workspace-root",
        &root_arg,
        "--rule",
        "custom_no_placeholder",
        "--llm",
        "--quiet",
    ]);
    assert!(llm.status.success(), "{}", text(&llm));
    assert!(text(&llm).contains("custom_no_placeholder"));

    let failing = run(&[
        "--workspace-root",
        &root_arg,
        "--rule",
        "custom_no_placeholder",
    ]);
    assert!(!failing.status.success());
    assert!(text(&failing).contains("custom_no_placeholder"));

    fs::write(
        root.join("src/lib.rs"),
        "// #rw(custom_no_placeholder) fixture policy exception\npub fn value() { /* CUSTOM_PLACEHOLDER */ }\n",
    )
    .expect("suppressed source should be written");
    let suppressed = run(&[
        "--workspace-root",
        &root_arg,
        "--rule",
        "custom_no_placeholder",
        "--quiet",
    ]);
    assert!(suppressed.status.success());

    let report = run(&[
        "--workspace-root",
        &root_arg,
        "--suppressions",
        "--quiet",
    ]);
    assert!(report.status.success());

    let clean = run(&[
        "--workspace-root",
        &root_arg,
        "--rule",
        "custom_no_placeholder",
        "--dry-run",
        "clean",
        "--quiet",
    ]);
    assert!(clean.status.success());
}
