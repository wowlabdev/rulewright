mod common;

use std::{fs, process::Command};

use common::{command, initialize, root_package, run, text, write};

#[test]
fn generated_config_disables_opt_in_rules() {
    let temporary = tempfile::tempdir().expect("temporary project should be created");
    let root = temporary.path();

    root_package(root, "config-defaults", "pub fn clean() {}\n");
    initialize(root);
    let contents = fs::read_to_string(root.join("rulewright.toml"))
        .expect("generated configuration should be readable");
    let config: toml::Value = toml::from_str(&contents).expect("generated configuration parses");

    assert_eq!(
        config["rules"]["rust_mutex_in_async"]["enabled"].as_bool(),
        Some(false)
    );
}

#[test]
fn root_package_is_discovered_from_a_descendant() {
    let temporary = tempfile::tempdir().expect("temporary project should be created");
    let root = temporary.path();

    root_package(
        root,
        "root-package",
        "#[unsafe(no_mangle)]\npub extern \"C\" fn exported() {}\npub fn probe() { dbg!(1); }\n",
    );
    initialize(root);

    let output = run(&root.join("src"), &["--rule", "rust_dbg"]);

    assert!(!output.status.success());
    assert!(text(&output).contains("src/lib.rs"));

    let package_aware = run(&root.join("src"), &["--rule", "rust_ffi_crate_naming"]);

    assert!(!package_aware.status.success());
    assert!(text(&package_aware).contains("root-package"));
}

#[test]
fn virtual_workspace_filters_nested_members_by_name_and_root() {
    let temporary = tempfile::tempdir().expect("temporary workspace should be created");
    let root = temporary.path();

    write(
        &root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"nested/one\", \"two\"]\nresolver = \"3\"\n",
    );
    root_package(
        &root.join("nested/one"),
        "nested-one",
        "pub fn one() { dbg!(1); }\n",
    );
    root_package(
        &root.join("two"),
        "member-two",
        "pub fn two() { dbg!(2); }\n",
    );
    initialize(root);

    for selector in ["nested-one", "nested/one"] {
        let output = run(root, &["--rule", "rust_dbg", "--filter", selector]);
        let output_text = text(&output);

        assert!(!output.status.success());
        assert!(output_text.contains("nested/one/src/lib.rs"));
        assert!(!output_text.contains("two/src/lib.rs"));
    }

    let unmatched = run(root, &["--rule", "rust_dbg", "--filter", "missing"]);

    assert!(!unmatched.status.success());
    assert!(text(&unmatched).contains("did not match a workspace member"));
}

#[test]
fn explicit_root_and_config_override_environment_discovery() {
    let temporary = tempfile::tempdir().expect("temporary projects should be created");
    let selected = temporary.path().join("selected");
    let environment = temporary.path().join("environment");
    let configuration = temporary.path().join("configuration/custom.toml");

    root_package(&selected, "selected", "pub fn selected() { dbg!(1); }\n");
    root_package(&environment, "environment", "pub fn clean() {}\n");
    fs::create_dir_all(configuration.parent().expect("configuration parent"))
        .expect("configuration directory should be created");

    let selected_argument = selected.to_string_lossy();
    let environment_argument = environment.to_string_lossy();
    let configuration_argument = configuration.to_string_lossy();
    let initialized = command(&environment)
        .env("RULEWRIGHT_ROOT", environment.as_os_str())
        .args([
            "--workspace-root",
            selected_argument.as_ref(),
            "--config",
            configuration_argument.as_ref(),
            "--init",
            "--quiet",
        ])
        .output()
        .expect("Rulewright init should run");

    assert!(initialized.status.success(), "{}", text(&initialized));
    assert!(configuration.is_file());
    assert!(!selected.join("rulewright.toml").exists());

    let output = command(&environment)
        .env("RULEWRIGHT_ROOT", environment_argument.as_ref())
        .args([
            "--workspace-root",
            selected_argument.as_ref(),
            "--config",
            configuration_argument.as_ref(),
            "--rule",
            "rust_dbg",
        ])
        .output()
        .expect("Rulewright should run");

    assert!(!output.status.success());
    assert!(text(&output).contains("src/lib.rs"));
}

#[test]
fn dirty_mode_analyzes_only_changed_sources_in_a_disposable_repository() {
    let temporary = tempfile::tempdir().expect("temporary repository should be created");
    let root = temporary.path();

    root_package(root, "dirty-fixture", "pub fn changed() {}\n");
    write(
        &root.join("src/committed.rs"),
        "pub fn committed() { dbg!(1); }\n",
    );
    initialize(root);

    for arguments in [
        ["init"].as_slice(),
        ["config", "user.email", "fixture@example.invalid"].as_slice(),
        ["config", "user.name", "Rulewright fixture"].as_slice(),
        ["add", "."].as_slice(),
        ["commit", "-m", "fixture baseline"].as_slice(),
    ] {
        let output = Command::new("git")
            .current_dir(root)
            .args(arguments)
            .output()
            .expect("fixture Git command should run");

        assert!(output.status.success(), "{}", text(&output));
    }

    write(&root.join("src/lib.rs"), "pub fn changed() { dbg!(2); }\n");
    let output = run(root, &["--rule", "rust_dbg", "--dirty"]);
    let output_text = text(&output);

    assert!(!output.status.success());
    assert!(output_text.contains("src/lib.rs"));
    assert!(!output_text.contains("src/committed.rs"));
}

#[test]
fn dirty_mode_retains_cross_file_workspace_findings_from_fresh_and_complete_cache() {
    let temporary = tempfile::tempdir().expect("temporary repository should be created");
    let root = temporary.path();
    let repeated = "a deliberately long repeated fixture string shared across files";

    root_package(root, "dirty-workspace-fixture", "pub fn changed() {}\n");
    write(
        &root.join("src/a.rs"),
        &format!("const A: &str = \"{repeated}\";\n"),
    );
    write(
        &root.join("src/b.rs"),
        &format!("const B: &str = \"{repeated}\";\n"),
    );
    initialize(root);

    for arguments in [
        ["init"].as_slice(),
        ["config", "user.email", "fixture@example.invalid"].as_slice(),
        ["config", "user.name", "Rulewright fixture"].as_slice(),
        ["add", "."].as_slice(),
        ["commit", "-m", "fixture baseline"].as_slice(),
    ] {
        let output = Command::new("git")
            .current_dir(root)
            .args(arguments)
            .output()
            .expect("fixture Git command should run");

        assert!(output.status.success(), "{}", text(&output));
    }

    write(
        &root.join("src/lib.rs"),
        &format!("const CHANGED: &str = \"{repeated}\";\n"),
    );

    for _ in 0..2 {
        let output = run(root, &["--rule", "rust_duplicate_strings", "--dirty"]);
        let output_text = text(&output);

        assert!(!output.status.success());
        assert!(output_text.contains("src/lib.rs"), "{output_text}");
        assert!(output_text.contains("src/b.rs"), "{output_text}");
    }
}

#[test]
fn llm_rejects_unknown_rule_instead_of_rendering_an_empty_catalog() {
    let temporary = tempfile::tempdir().expect("temporary directory should be created");
    let output = run(
        temporary.path(),
        &["--llm", "--rule", "definitely_not_a_rule"],
    );
    let output_text = text(&output);

    assert!(!output.status.success());
    assert!(output_text.contains("unknown rule: definitely_not_a_rule"));
    assert!(!output_text.contains("# rulewright"));
}

#[test]
fn llm_explains_how_to_use_alignment_regions() {
    let temporary = tempfile::tempdir().expect("temporary project should be created");
    let root = temporary.path();

    root_package(root, "alignment-guide", "pub fn clean() {}\n");
    initialize(root);

    let output = run(root, &["--llm", "--rule", "rust_aligned"]);
    let output_text = text(&output);

    assert!(output.status.success(), "{output_text}");
    assert!(output_text.contains("## Alignment guide"));
    assert!(output_text.contains("`// #rw:aligned` is not a suppression"));
    assert!(output_text.contains("corresponding commas"));
    assert!(output_text.contains("#[rustfmt::skip]"));
    assert!(output_text.contains("reported without an automatic rewrite"));
}

#[test]
fn terminal_action_conflicts_fail_before_any_action_runs() {
    let temporary = tempfile::tempdir().expect("temporary directory should be created");

    for arguments in [
        ["--ci", "--llm"].as_slice(),
        ["--fix", "--llm"].as_slice(),
        ["--list", "--init"].as_slice(),
    ] {
        let output = run(temporary.path(), arguments);

        assert!(!output.status.success());
        assert!(text(&output).contains("cannot be used with"));
    }
}

#[test]
fn discovery_failure_is_contextual() {
    let temporary = tempfile::tempdir().expect("temporary directory should be created");
    let output = run(temporary.path(), &["--rule", "rust_dbg"]);

    assert!(!output.status.success());
    assert!(text(&output).contains("failed to discover a Cargo root"));
}
