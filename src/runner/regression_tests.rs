#![allow(
    clippy::assert_is_empty,
    reason = "fixture assertions use explicit collection emptiness"
)]
use super::*;
use crate::{RuleInfo, RulePack, Severity, directory, temporary::Directory};

fn production(ctx: &FileCtx<'_>) -> Vec<Violation> {
    (!ctx.test_only)
        .then(|| crate::violation(ctx.rel, 1, "production"))
        .into_iter()
        .collect()
}

fn changed(ctx: &FileCtx<'_>) -> Vec<Violation> {
    vec![crate::violation(ctx.rel, 1, "changed pack")]
}

fn inventory(ctx: &WorkspaceCtx<'_>) -> Vec<Violation> {
    ctx.paths
        .iter()
        .filter(|path| {
            Path::new(path)
                .extension()
                .is_some_and(|extension| extension == "py")
        })
        .map(|path| crate::violation(path, 1, "foreign file"))
        .collect()
}

const INFO: RuleInfo = RuleInfo::new(
    "fixture_kind",
    "Report production files.",
    "Exercise classification cache inputs.",
    Severity::Low,
    &[],
    &[],
);
static ORIGINAL: &[Rule] = &[Rule::rust_line_full(INFO, production, None)];
static CHANGED: &[Rule] = &[Rule::rust_line_full(INFO, changed, None)];
static INVENTORY: &[Rule] = &[Rule::workspace(
    RuleInfo::new(
        "fixture_inventory",
        "Report foreign files.",
        "Exercise inventory cache inputs.",
        Severity::Low,
        &[],
        &[],
    ),
    inventory,
)];

fn registry(rules: &'static [Rule], fingerprint: &'static str) -> RuleRegistry {
    let mut registry = RuleRegistry::with_builtins().unwrap();

    registry
        .extend(RulePack {
            name: "fixture",
            version: "1.0.0",
            implementation_fingerprint: fingerprint,
            rules,
        })
        .unwrap();
    registry
}

fn fixture() -> Directory {
    let temp = Directory::new().unwrap();

    directory::ensure(&temp.path().join("src")).unwrap();
    file::write_text(
        &temp.path().join("Cargo.toml"),
        "[package]\nname='fixture'\nversion='0.1.0'\nedition='2024'\n",
    )
    .unwrap();
    file::write_text(&temp.path().join("src/lib.rs"), "pub fn clean() {}\n").unwrap();
    temp
}

fn scan(
    root: &Path,
    registry: &RuleRegistry,
    config: &Config,
    packages: &[PackageRoot],
    fresh: bool,
) -> Vec<(String, String)> {
    let rule_filter = vec!["fixture_kind".to_owned(), "fixture_inventory".to_owned()];
    let ctx = RunCtx {
        registry,
        config,
        packages,
        rule_filter: &rule_filter,
        package_filter: &[],
        root,
        workspace_root: root,
        quiet: true,
        dirty: false,
    };

    collect_violations(&ctx, fresh, None)
        .unwrap()
        .0
        .into_iter()
        .map(|finding| (finding.rel, finding.message))
        .collect()
}

#[test]
fn cached_results_track_pack_config_manifest_and_test_kind() {
    let temp = fixture();
    let root = temp.path();
    let original = registry(ORIGINAL, "v1");
    let updated = registry(CHANGED, "v2");
    let mut config = Config::generate_default(&[("fixture_kind", &[])]);

    assert_eq!(scan(root, &original, &config, &[], false).len(), 1);
    assert_eq!(
        scan(root, &original, &config, &[], false),
        scan(root, &original, &config, &[], true)
    );
    assert_eq!(
        scan(root, &updated, &config, &[], false)[0].1,
        "changed pack"
    );
    config.rules.get_mut("fixture_kind").unwrap().only = vec!["tests/**".to_owned()];
    assert!(scan(root, &updated, &config, &[], false).is_empty());
    config.rules.get_mut("fixture_kind").unwrap().only.clear();
    let mut packages = vec![PackageRoot {
        name: "fixture".to_owned(),
        root: root.to_path_buf(),
        publishable: false,
        test_only: true,
        test_targets: Vec::new(),
    }];

    assert!(scan(root, &original, &config, &packages, false).is_empty());
    packages[0].test_only = false;
    assert_eq!(scan(root, &original, &config, &packages, false).len(), 1);
    file::write_text(&root.join("src/lib.rs"), "#[cfg(test)] mod helper;\n").unwrap();
    file::write_text(&root.join("src/helper.rs"), "pub fn helper() {}\n").unwrap();
    assert_eq!(scan(root, &original, &config, &packages, false).len(), 1);
    file::write_text(&root.join("src/lib.rs"), "mod helper;\n").unwrap();
    assert_eq!(scan(root, &original, &config, &packages, false).len(), 2);
    file::write_text(
        &root.join("Cargo.toml"),
        "[package]\nname='fixture'\nversion='0.2.0'\nedition='2024'\n",
    )
    .unwrap();
    assert_eq!(
        scan(root, &original, &config, &packages, false),
        scan(root, &original, &config, &packages, true)
    );
}

#[test]
fn cached_inventory_tracks_new_renamed_and_deleted_foreign_files() {
    let temp = fixture();
    let root = temp.path();
    let registry = registry(INVENTORY, "inventory");
    let config = Config::generate_default(&[("fixture_inventory", &[])]);

    assert!(scan(root, &registry, &config, &[], false).is_empty());
    file::write_text(&root.join("script.py"), "print(1)").unwrap();
    assert_eq!(scan(root, &registry, &config, &[], false)[0].0, "script.py");
    std::fs::rename(root.join("script.py"), root.join("renamed.py")).unwrap();
    assert_eq!(
        scan(root, &registry, &config, &[], false)[0].0,
        "renamed.py"
    );
    std::fs::remove_file(root.join("renamed.py")).unwrap();
    assert!(scan(root, &registry, &config, &[], false).is_empty());
}

#[test]
fn dirty_inventory_findings_survive_an_empty_rust_target_set() {
    let temp = fixture();
    let root = temp.path();
    let registry = registry(INVENTORY, "inventory");
    let config = Config::generate_default(&[("fixture_inventory", &[])]);

    file::write_text(&root.join("script.py"), "print(1)").unwrap();
    assert_eq!(scan(root, &registry, &config, &[], false).len(), 1);

    let rules = [&INVENTORY[0]];
    let ctx = RunCtx {
        registry: &registry,
        config: &config,
        packages: &[],
        rule_filter: &[],
        package_filter: &[],
        root,
        workspace_root: root,
        quiet: true,
        dirty: true,
    };
    let cache = cache::Session::open(&ctx, &rules).unwrap();
    let targets = HashSet::new();
    let result = complete_cache_result(Some(&cache), &rules, Some(&targets), true).unwrap();

    assert_eq!(result.0.len(), 1);

    let mut findings =
        vec![crate::violation("script.py", 1, "foreign file").with_rule("fixture_inventory")];

    retain_targets(
        &rules,
        Some(&targets),
        true,
        &mut findings,
        &mut Vec::new(),
        &mut Vec::new(),
    );
    assert_eq!(findings.len(), 1);
}
