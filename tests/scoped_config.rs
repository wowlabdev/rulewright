#![allow(
    clippy::assert_is_empty,
    reason = "fixture assertions use explicit collection emptiness"
)]
mod common;

use common::{initialize, root_package, run, text, write};
use rulewright::{Config, Path, RuleRegistry};

fn load(source: &str) -> Config {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("rulewright.toml");

    write(&path, source);
    Config::load(Path::new(&path)).unwrap()
}

#[test]
fn scopes_expand_sets_normalize_windows_and_merge_in_order() {
    let config = load(
        r#"
[glob_sets]
tests = ["tests/**"]
[rules.rust_too_many_lines_in_file]
enabled = true
only = ["src/**", "@tests"]
ignore = ["tests/generated/**"]
threshold = 2
slack_percent = 0
[[rules.rust_too_many_lines_in_file.overrides]]
paths = ["@tests"]
threshold = 9
[[rules.rust_too_many_lines_in_file.overrides]]
paths = ["tests/small.rs"]
threshold = 4
"#,
    );
    let registry = RuleRegistry::with_builtins().unwrap();
    let metadata = registry.metadata();
    let rule = metadata
        .iter()
        .find(|rule| rule.name == "rust_too_many_lines_in_file")
        .unwrap();

    assert!(config.validate(&[(rule.name, rule.params)]).0.is_empty());
    assert!(config.applies_to(rule.name, "tests/big.rs"));
    assert!(!config.applies_to(rule.name, "tests/generated/file.rs"));
    assert!(!config.applies_to(rule.name, "examples/demo.rs"));

    for (path, expected) in [
        ("src/lib.rs", 2),
        ("tests/big.rs", 9),
        ("tests/small.rs", 4),
    ] {
        assert_eq!(config.get_usize(rule.name, &rule.params[0], path), expected);
    }

    let json = serde_json::to_value(config.resolved_rules(&metadata)).unwrap();
    let resolved = json
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == rule.name)
        .unwrap();

    assert_eq!(resolved["overrides"][0]["paths"][0], "tests/**");
    assert_eq!(resolved["only"].as_array().unwrap().len(), 2);
}

#[test]
fn invalid_scope_and_override_configuration_is_rejected() {
    let empty_set = tempfile::tempdir().unwrap();
    let empty_path = empty_set.path().join("rulewright.toml");

    write(
        &empty_path,
        "[glob_sets]\nempty=[]\n[rules.rust_max_fn_lines]\nenabled=true\nthreshold=4\n[[rules.rust_max_fn_lines.overrides]]\npaths=['@empty']\nthreshold=8",
    );
    assert!(Config::load(Path::new(&empty_path)).is_err());

    for suffix in [
        "only = ['@missing']",
        "only = ['[']",
        "[[rules.rust_max_fn_lines.overrides]]\npaths = []",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("rulewright.toml");

        write(
            &path,
            &format!("[rules.rust_max_fn_lines]\nenabled = true\nthreshold = 4\n{suffix}\n"),
        );
        assert!(Config::load(Path::new(&path)).is_err(), "{suffix}");
    }

    let registry = RuleRegistry::with_builtins().unwrap();
    let metadata = registry.metadata();
    let rule = metadata
        .iter()
        .find(|rule| rule.name == "rust_max_fn_lines")
        .unwrap();

    for invalid in [
        "threshold = -1",
        "threshold = 'large'",
        "enabled = false",
        "typo = 4",
    ] {
        let config = load(&format!(
            "[rules.rust_max_fn_lines]\nenabled = true\nthreshold = 4\n[[rules.rust_max_fn_lines.overrides]]\npaths = ['tests/**']\n{invalid}\n"
        ));

        assert!(
            !config.validate(&[(rule.name, rule.params)]).0.is_empty(),
            "{invalid}"
        );
    }
}

#[test]
fn line_and_ast_dispatch_honor_only_and_overrides() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    root_package(root, "scoped", "pub fn demo() {\n dbg!(1);\n dbg!(2);\n}\n");
    write(&root.join("src/other.rs"), "pub fn other() { dbg!(3); }\n");
    initialize(root);
    let config_path = root.join("rulewright.toml");
    let original = std::fs::read_to_string(&config_path)
        .unwrap()
        .replace("only = []\n", "")
        .replace("overrides = []\n", "");
    let configured = original
        .replace(
            "[rules.rust_dbg]",
            "[rules.rust_dbg]\nonly = ['src/lib.rs']",
        )
        .replace(
            "[rules.rust_too_many_lines_in_file]",
            "[rules.rust_too_many_lines_in_file]\nonly = ['src/lib.rs']",
        );

    write(
        &config_path,
        &format!(
            "{configured}\n[[rules.rust_too_many_lines_in_file.overrides]]\npaths = ['src/lib.rs']\nthreshold = 1\nslack_percent = 0\n"
        ),
    );
    let output = run(root, &["--rule", "rust_dbg", "--format", "json"]);

    assert!(!output.status.success(), "{}", text(&output));
    assert!(text(&output).contains("src/lib.rs"));
    assert!(!text(&output).contains("src/other.rs"));
    let output = run(root, &["--rule", "rust_too_many_lines_in_file"]);

    assert!(!output.status.success(), "{}", text(&output));
    assert!(text(&output).contains("src/lib.rs"));
}

#[test]
fn baseline_forbid_rejects_stale_entries_and_requires_baseline() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    root_package(root, "baseline-policy", "pub fn clean() {}\n");
    initialize(root);
    write(
        &root.join("baseline.json"),
        r#"{"schema_version":1,"findings":[{"rule":"rust_dbg","path":"deleted.rs","message":"old finding","count":0}]}"#,
    );
    let output = run(
        root,
        &[
            "--rule",
            "rust_dbg",
            "--baseline",
            "baseline.json",
            "--baseline-forbid",
            "rust_dbg,rust_panic",
        ],
    );

    assert!(!output.status.success());
    assert!(text(&output).contains("forbidden rule `rust_dbg`"));
    assert!(
        run(
            root,
            &[
                "--rule",
                "rust_dbg",
                "--baseline",
                "baseline.json",
                "--baseline-forbid",
                "rust_panic"
            ]
        )
        .status
        .success()
    );
    assert!(
        !run(root, &["--baseline-forbid", "rust_dbg"])
            .status
            .success()
    );
}

#[test]
fn dirty_policy_changes_equal_full_results_with_warm_cache() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    root_package(root, "dirty-policy", "pub fn demo() { dbg!(1); }\n");
    initialize(root);
    let path = root.join("rulewright.toml");
    let original = std::fs::read_to_string(&path)
        .unwrap()
        .replace("only = []\n", "")
        .replace("overrides = []\n", "");

    write(
        &path,
        &original.replace("[rules.rust_dbg]", "[rules.rust_dbg]\nonly = ['tests/**']"),
    );

    for args in [
        vec!["init"],
        vec!["config", "user.email", "fixture@example.invalid"],
        vec!["config", "user.name", "Fixture"],
        vec!["add", "."],
        vec!["commit", "-m", "fixture"],
    ] {
        assert!(
            std::process::Command::new("git")
                .current_dir(root)
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }

    assert!(
        run(root, &["--rule", "rust_dbg", "--format", "json"])
            .status
            .success()
    );
    write(&path, &original);
    let dirty = run(root, &["--rule", "rust_dbg", "--format", "json", "--dirty"]);
    let full = run(root, &["--rule", "rust_dbg", "--format", "json"]);

    assert!(!dirty.status.success(), "{}", text(&dirty));
    assert!(
        !run(root, &["--rule", "rust_dbg", "--dirty"])
            .status
            .success()
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&dirty.stdout).unwrap()["findings"],
        serde_json::from_slice::<serde_json::Value>(&full.stdout).unwrap()["findings"]
    );
    // Changing only Cargo's test-kind declaration must also reclassify cached Rust files.
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();

    write(
        &root.join("Cargo.toml"),
        &format!("{manifest}\n[package.metadata.rulewright]\ntest-only=true\n"),
    );
    let dirty = run(root, &["--rule", "rust_dbg", "--format", "json", "--dirty"]);
    let full = run(root, &["--rule", "rust_dbg", "--format", "json"]);

    assert!(dirty.status.success(), "{}", text(&dirty));
    assert_eq!(dirty.stdout, full.stdout);
}

#[test]
fn manifest_policy_change_cannot_reuse_cached_success() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    root_package(root, "manifest-cache", "pub fn clean() {}\n");
    initialize(root);
    let args = &["--rule", "toml_cargo_layers", "--format", "json"];
    let config_path = root.join("rulewright.toml");
    let original = std::fs::read_to_string(&config_path)
        .unwrap()
        .replace("only = []\n", "")
        .replace("overrides = []\n", "");

    write(
        &config_path,
        &original.replace("rules = []", "rules = ['Cargo.toml !> forbidden']"),
    );
    assert!(run(root, args).status.success());
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();

    write(
        &root.join("Cargo.toml"),
        &format!("{manifest}\n[dependencies]\nforbidden='1'\n"),
    );
    let changed = run(root, args);

    assert!(!changed.status.success(), "{}", text(&changed));
    assert!(text(&changed).contains("violates layer edge"));
    write(&root.join("Cargo.toml"), &manifest);
    assert!(run(root, args).status.success());
}
