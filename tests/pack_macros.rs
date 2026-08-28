use rulewright::{
    AstCtx, Example, FileCtx, RulePack, RuleRegistry, TomlCtx, Violation, WorkspaceCtx,
};

const EXAMPLES: &[Example] = &[];

rulewright::pack_line_rule!(
    first_fixture,
    "First fixture rule.",
    "Exercises parameter namespacing.",
    Low,
    params {
        labels: [String] = ["one"] in ["one", "two"],
        threshold: i64 = 2,
    },
);

rulewright::pack_line_rule!(
    second_fixture,
    "Second fixture rule.",
    "Exercises parameter namespacing.",
    Medium,
    default = false,
    params {
        labels: [String] = ["one", "two"]
    },
);

rulewright::pack_ast_rule!(
    ast_fixture_opt_in,
    "Opt-in AST fixture rule.",
    "Exercises default-disabled AST pack registration.",
    Low,
    default = false,
);

rulewright::pack_toml_rule!(
    toml_fixture_opt_in,
    "Opt-in fixture rule.",
    "Exercises default-disabled TOML registration.",
    Low,
    default = false,
);

rulewright::pack_line_rule!(
    negative_default_fixture,
    "Negative-default fixture rule.",
    "Exercises signed integer parsing before metadata validation.",
    Low,
    params {
        threshold: i64 = -5
    },
);

rulewright::pack_line_rule!(
    invalid_allowed_default_fixture,
    "Invalid allowed-default fixture rule.",
    "Exercises allowed-value metadata validation.",
    Low,
    params { labels: [String] = ["missing"] in ["present"] },
);

static NEGATIVE_DEFAULT_RULES: &[rulewright::Rule] = &[NEGATIVE_DEFAULT_FIXTURE_RULE];
static INVALID_ALLOWED_DEFAULT_RULES: &[rulewright::Rule] = &[INVALID_ALLOWED_DEFAULT_FIXTURE_RULE];

rulewright::line_rule!(
    inventory_fixture_opt_in,
    "Opt-in inventory fixture rule.",
    "Exercises disabled line rules with parameters.",
    Low,
    default = false,
    params { threshold: i64 = 1 },
);

rulewright::full_line_rule!(
    full_inventory_fixture_opt_in,
    "Opt-in full-line fixture rule.",
    "Exercises disabled full-line registration.",
    Low,
    default = false,
);

rulewright::full_line_rule!(
    full_low_fixture,
    "Low-severity full-line fixture rule.",
    "Exercises the default-severity shorthand.",
);

rulewright::full_line_rule!(
    full_low_params_fixture,
    "Parameterized low-severity full-line fixture rule.",
    "Exercises the parameterized default-severity shorthand.",
    params { threshold: i64 = 1 },
);

rulewright::workspace_rule!(
    workspace_fixture,
    "Bare workspace fixture rule.",
    "Exercises workspace registration without parameters.",
    Low,
);

rulewright::language_workspace_rule!(
    generic_workspace_fixture,
    "Bare generic workspace fixture rule.",
    "Exercises language-neutral registration without parameters.",
    Low,
);

rulewright::toml_rule!(
    toml_low_fixture,
    "Low-severity TOML fixture rule.",
    "Exercises the default-severity shorthand.",
);

fn check_first_fixture(_: &FileCtx<'_>) -> Vec<Violation> {
    Vec::new()
}

fn check_second_fixture(_: &FileCtx<'_>) -> Vec<Violation> {
    Vec::new()
}

fn check_ast_fixture_opt_in(_: &AstCtx<'_>) -> Vec<Violation> {
    Vec::new()
}

fn check_toml_fixture_opt_in(_: &TomlCtx<'_>) -> Vec<Violation> {
    Vec::new()
}

fn check_negative_default_fixture(_: &FileCtx<'_>) -> Vec<Violation> {
    Vec::new()
}

fn check_invalid_allowed_default_fixture(_: &FileCtx<'_>) -> Vec<Violation> {
    Vec::new()
}

fn check_inventory_fixture_opt_in(_: &FileCtx<'_>) -> Vec<Violation> {
    Vec::new()
}

fn check_full_inventory_fixture_opt_in(_: &FileCtx<'_>) -> Vec<Violation> {
    Vec::new()
}

fn check_full_low_fixture(_: &FileCtx<'_>) -> Vec<Violation> {
    Vec::new()
}

fn check_full_low_params_fixture(_: &FileCtx<'_>) -> Vec<Violation> {
    Vec::new()
}

fn check_workspace_fixture(_: &WorkspaceCtx<'_>) -> Vec<Violation> {
    Vec::new()
}

fn check_generic_workspace_fixture(_: &WorkspaceCtx<'_>) -> Vec<Violation> {
    Vec::new()
}

fn check_toml_low_fixture(_: &TomlCtx<'_>) -> Vec<Violation> {
    Vec::new()
}

#[test]
fn multiple_parameterized_pack_rules_share_one_module() {
    assert_eq!(FIRST_FIXTURE_PARAMS[0].name, "labels");
    assert_eq!(FIRST_FIXTURE_PARAMS[0].allowed_values, ["one", "two"]);
    assert_eq!(FIRST_FIXTURE_PARAMS[1].name, "threshold");
    assert_eq!(SECOND_FIXTURE_PARAMS[0].name, "labels");
    assert_eq!(FIRST_FIXTURE_RULE.id(), "rust_first_fixture");
    assert_eq!(SECOND_FIXTURE_RULE.id(), "rust_second_fixture");
}

#[test]
fn pack_rules_can_be_disabled_by_default() {
    assert!(!SECOND_FIXTURE_RULE.info.default_enabled);
    assert!(!AST_FIXTURE_OPT_IN_RULE.info.default_enabled);
    assert_eq!(TOML_FIXTURE_OPT_IN_RULE.id(), "toml_fixture_opt_in");
    assert!(!TOML_FIXTURE_OPT_IN_RULE.info.default_enabled);
}

#[test]
fn registry_validates_parameter_values_after_macro_parsing() {
    for (rules, message) in [
        (
            NEGATIVE_DEFAULT_RULES,
            "integer parameter defaults must be non-negative",
        ),
        (
            INVALID_ALLOWED_DEFAULT_RULES,
            "string-array defaults must use allowed values",
        ),
    ] {
        let mut registry = RuleRegistry::with_builtins().expect("fixture registry");
        let error = registry
            .extend(RulePack {
                name: "fixture",
                version: "0.1.0",
                implementation_fingerprint: "fixture:v1",
                rules,
            })
            .expect_err("invalid parameter metadata");

        assert!(error.to_string().contains(message));
    }
}

#[test]
fn inventory_macros_need_no_transitive_dependencies_or_dummy_params() {
    let registry = RuleRegistry::with_builtins().expect("fixture registry");

    for id in [
        "rust_inventory_fixture_opt_in",
        "rust_full_inventory_fixture_opt_in",
    ] {
        let rule = registry
            .rules()
            .iter()
            .find(|rule| rule.id() == id)
            .expect("fixture rule");

        assert!(!rule.info.default_enabled);
    }

    assert!(
        registry
            .rules()
            .iter()
            .any(|rule| rule.id() == "rust_workspace_fixture")
    );
    assert!(
        registry
            .rules()
            .iter()
            .any(|rule| rule.id() == "generic_workspace_fixture")
    );
    assert_eq!(
        registry
            .rules()
            .iter()
            .find(|rule| rule.id() == "toml_low_fixture")
            .expect("TOML fixture rule")
            .info
            .severity,
        rulewright::Severity::Low
    );

    for id in ["rust_full_low_fixture", "rust_full_low_params_fixture"] {
        assert_eq!(
            registry
                .rules()
                .iter()
                .find(|rule| rule.id() == id)
                .expect("full-line fixture rule")
                .info
                .severity,
            rulewright::Severity::Low
        );
    }
}

#[cfg(feature = "rule-pack-testing")]
mod generated_test_modules {
    use super::*;

    fn check_line(_: &FileCtx<'_>) -> Vec<Violation> {
        Vec::new()
    }

    fn check_ast(_: &AstCtx<'_>) -> Vec<Violation> {
        Vec::new()
    }

    rulewright::rulewright_test!(check_line, {
        rulewright::example_tests!(EXAMPLES, check_line);
    });
    rulewright::rulewright_ast_test!(check_ast, {
        rulewright::example_tests!(EXAMPLES, check_ast);
    });
}
