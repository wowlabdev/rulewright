#![allow(
    clippy::assert_is_empty,
    reason = "fixture assertions use explicit collection emptiness"
)]
#![cfg(feature = "rule-pack-testing")]

use rulewright::{
    Config, DependencySection, DependencySource, Path, RuleCheck, RuleRegistry, Violation,
    WorkspaceCtx,
};

fn config(rule: &str, params: &str) -> Config {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("rulewright.toml");

    std::fs::write(&path, format!("[rules.{rule}]\nenabled = true\n{params}\n")).unwrap();
    Config::load(Path::new(&path)).unwrap()
}

fn check(rule: &str, params: &str, sources: &[(&str, &str)]) -> Vec<Violation> {
    let registry = RuleRegistry::with_builtins().unwrap();
    let registered = registry
        .rules()
        .iter()
        .find(|entry| entry.info.name == rule)
        .unwrap();
    let (RuleCheck::Workspace(check) | RuleCheck::RustWorkspace(check)) = registered.check else {
        panic!("workspace rule required")
    };
    let config = config(rule, params);

    assert!(
        config
            .validate(&[(rule, registered.info.params)])
            .0
            .is_empty()
    );
    rulewright::testing::check_workspace_sources_with_config(sources, &config, check)
}

#[test]
fn workspace_parameters_use_each_finding_path() {
    let sources = [
        ("a.rs", "pub fn a() { consume(\"shared string\"); }"),
        ("b.rs", "pub fn b() { consume(\"shared string\"); }"),
    ];
    let base = "min_chars=3\nmin_occurrences=3";

    assert!(check("rust_duplicate_strings", base, &sources).is_empty());

    let configured = format!(
        "{base}\n[[rules.rust_duplicate_strings.overrides]]\npaths=['b.rs']\nmin_occurrences=2"
    );
    let findings = check("rust_duplicate_strings", &configured, &sources);

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rel, "b.rs");
}

#[test]
fn reserved_config_keys_cannot_be_pack_parameters() {
    static PARAMS: &[rulewright::RuleParam] = &[rulewright::RuleParam {
        name: "only",
        param_type: rulewright::ParamType::StringArray,
        default: rulewright::ParamDefault::StringArray(&[]),
        allowed_values: &[],
    }];
    static RULES: &[rulewright::Rule] = &[rulewright::Rule::workspace(
        rulewright::RuleInfo::new(
            "fixture_reserved",
            "Reserved parameter.",
            "Reject ambiguous config keys.",
            rulewright::Severity::Low,
            &[],
            PARAMS,
        ),
        check_fixture_disabled,
    )];
    let mut registry = RuleRegistry::with_builtins().unwrap();
    let error = registry
        .extend(rulewright::RulePack {
            name: "reserved",
            version: "1.0.0",
            implementation_fingerprint: "v1",
            rules: RULES,
        })
        .unwrap_err();

    assert!(error.to_string().contains("reserved"));
}

#[test]
fn manifest_inventory_preserves_all_sections_sources_and_locations() {
    fn inspect(ctx: &WorkspaceCtx<'_>) -> Vec<Violation> {
        assert_eq!(ctx.paths, &["Cargo.toml", "member/Cargo.toml", "script.py"]);
        let root = &ctx.manifests[0];

        assert_eq!(root.package_name, None);
        assert_eq!(
            root.patch_tables["crates-io"][0].source,
            DependencySource::Path("vendor/dep".to_owned())
        );
        assert_eq!(
            root.workspace_dependencies[0].package.as_deref(),
            Some("actual")
        );
        let member = &ctx.manifests[1];

        assert_eq!(member.package_name.as_deref(), Some("member"));
        let deps = &member.dependencies;

        assert_eq!(deps.len(), 5);
        assert_eq!(deps[0].source, DependencySource::Workspace);
        assert_eq!(deps[0].section, DependencySection::Normal);
        assert_eq!(deps[1].source, DependencySource::Git);
        assert_eq!(deps[1].section, DependencySection::Dev);
        assert_eq!(deps[2].section, DependencySection::Build);
        assert_eq!(deps[2].package.as_deref(), Some("compiler"));
        assert_eq!(deps[3].line, 11);
        assert_eq!(deps[4].line, 13);
        Vec::new()
    }
    let findings = rulewright::testing::check_workspace_sources(
        &[
            ("script.py", "print('inventory')"),
            (
                "Cargo.toml",
                "[workspace]\nmembers=[]\n[workspace.dependencies]\nalias={ package='actual', version='1' }\n[patch.crates-io]\ndep={path='vendor/dep'}",
            ),
            (
                "member/Cargo.toml",
                "[package]\nname='member'\n[dependencies]\nalias.workspace=true\n[dev-dependencies]\ngitdep={git='https://example.invalid/repo'}\n[build-dependencies.tool]\npackage='compiler'\nversion='1'\n[target.'cfg(unix)'.dependencies]\nplatform='1'\n[target.'cfg(windows)'.dependencies]\nplatform='2'",
            ),
        ],
        inspect,
    );

    assert!(findings.is_empty());
}

#[test]
fn layer_rules_cover_renamed_inherited_dev_build_and_target_edges() {
    let root = "[workspace]\nmembers=[]\n[workspace.dependencies]\nalias={package='engine-core',version='1'}";
    let policy = "rules = ['server/** !> engine-*,host/**']";

    for declaration in [
        "[dependencies]\nengine-core='1'",
        "[dev-dependencies]\nalias={package='engine-core',version='1'}",
        "[build-dependencies]\nalias.workspace=true",
        "[target.'cfg(unix)'.dependencies]\nalias.workspace=true",
        "[dependencies]\nhost-api='1'",
    ] {
        let source = format!("[package]\nname='service'\n{declaration}");
        let findings = check(
            "toml_cargo_layers",
            policy,
            &[
                ("Cargo.toml", root),
                ("server/Cargo.toml", &source),
                ("host/api/Cargo.toml", "[package]\nname='host-api'"),
            ],
        );

        assert_eq!(findings.len(), 1, "{declaration}: {findings:?}");
    }

    assert!(
        check(
            "toml_cargo_layers",
            policy,
            &[("host/Cargo.toml", "[dependencies]\nengine-core='1'")]
        )
        .is_empty()
    );

    for invalid in [
        "rules=['bad']",
        "rules=['server/** !> ']",
        "rules=['[ !> dep']",
    ] {
        let cfg = config("toml_cargo_layers", invalid);
        let registry = RuleRegistry::with_builtins().unwrap();
        let rule = registry
            .rules()
            .iter()
            .find(|rule| rule.info.name == "toml_cargo_layers")
            .unwrap();

        assert!(
            !cfg.validate(&[(rule.info.name, rule.info.params)])
                .0
                .is_empty()
        );
    }
}

#[test]
fn path_policy_handles_virtual_roots_patches_target_tables_and_lexical_escape() {
    let restrictive = "allow_in=['Cargo.toml']\nallow_outside_workspace=false\nallow_patch=false";

    for (path, source, count) in [
        (
            "Cargo.toml",
            "[workspace.dependencies]\ndep={path='crates/dep'}",
            0,
        ),
        (
            "Cargo.toml",
            "[workspace.dependencies]\ndep={path='../dep'}",
            1,
        ),
        (
            "Cargo.toml",
            "[patch.crates-io]\ndep={path='vendor/dep'}",
            1,
        ),
        (
            "member/Cargo.toml",
            "[target.'cfg(unix)'.dev-dependencies]\ndep={path='../dep'}",
            1,
        ),
        ("member/Cargo.toml", "[dependencies]\ndep.workspace=true", 0),
    ] {
        assert_eq!(
            check("toml_cargo_path_deps", restrictive, &[(path, source)]).len(),
            count
        );
    }

    let allowed = "allow_in=['**/Cargo.toml']\nallow_outside_workspace=false\nallow_patch=true";

    for (path, count) in [
        ("../peer", 0),
        ("../../outside", 1),
        ("/tmp/absolute", 1),
        ("C:\\outside", 1),
        ("nested/../../peer", 0),
    ] {
        let source = format!("[patch.crates-io]\ndep={{path='{path}'}}");

        assert_eq!(
            check(
                "toml_cargo_path_deps",
                allowed,
                &[("member/Cargo.toml", &source)]
            )
            .len(),
            count,
            "{path}"
        );
    }

    assert!(
        check(
            "toml_cargo_path_deps",
            "allow_in=['Cargo.toml']\nallow_outside_workspace=true\nallow_patch=true",
            &[("Cargo.toml", "[patch.crates-io]\ndep={path='../dep'}")]
        )
        .is_empty()
    );
}

const EXAMPLES: &[rulewright::Example] = &[];
rulewright::pack_workspace_rule!(
    fixture_inventory,
    "Inventory fixture.",
    "Verify public pack registration and workspace fixtures.",
    Low,
    params {
        minimum: i64 = 1,
        strict: bool = true
    }
);
rulewright::pack_workspace_rule!(
    fixture_disabled,
    "Disabled fixture.",
    "Verify opt-in workspace pack registration.",
    Low,
    default = false
);

fn check_fixture_inventory(ctx: &WorkspaceCtx<'_>) -> Vec<Violation> {
    assert!(ctx.config.get_bool(
        "fixture_inventory",
        &FIXTURE_INVENTORY_PARAMS[1],
        "script.py"
    ));
    ctx.paths
        .iter()
        .filter(|path| {
            Path::new(path)
                .extension()
                .is_some_and(|extension| extension == "py")
        })
        .map(|path| rulewright::violation(path, 1, "script fixture"))
        .collect()
}

fn check_fixture_disabled(_: &WorkspaceCtx<'_>) -> Vec<Violation> {
    Vec::new()
}

rulewright::rulewright_workspace_test!(check_fixture_inventory, {
    #[test]
    fn public_workspace_fixture_supports_foreign_paths() {
        assert!(run("pub fn clean() {}\n").is_empty());
        assert_eq!(
            run_files(&[("script.py", ""), ("Cargo.toml", "[workspace]")]).len(),
            1
        );
        assert!(matches!(
            FIXTURE_INVENTORY_RULE.check,
            RuleCheck::Workspace(_)
        ));
        assert!(!FIXTURE_DISABLED_RULE.info.default_enabled);
    }
});
