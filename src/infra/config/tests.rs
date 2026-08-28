use crate::{directory, temporary::Directory};
use googletest::prelude::*;

use super::*;

fn parse(toml_str: &str) -> Config {
    let raw: RawConfig = toml::from_str(toml_str).unwrap();

    Config::resolve(raw).unwrap()
}

#[gtest]
fn basic_config() -> Result<()> {
    let cfg = parse(
        r#"
[glob_sets]
tests = ["**/tests/**"]

[rules.panic]
enabled = true
ignore = ["tests"]

[rules.style]
enabled = false
"#,
    );

    verify_that!(cfg.is_enabled("panic"), is_true())?;
    verify_that!(cfg.is_enabled("style"), is_false())?;

    verify_that!(cfg.ignore_patterns("panic"), eq(&["**/tests/**"]))
}

#[gtest]
fn source_suppressions_are_allowed_by_default_and_can_be_disabled() -> Result<()> {
    let default = parse("[rules.panic]\nenabled = true\n");
    let disabled = parse("allow_suppressions = false\n\n[rules.panic]\nenabled = true\n");

    verify_true!(default.allows_suppressions())?;

    verify_false!(disabled.allows_suppressions())
}

#[gtest]
fn multiple_glob_sets() -> Result<()> {
    let cfg = parse(
        r#"
[glob_sets]
tests = ["**/tests/**", "**/benches/**"]
binary = ["**/main.rs", "**/bin/**"]

[rules.panic]
enabled = true
ignore = ["tests", "binary"]
"#,
    );
    let patterns = cfg.ignore_patterns("panic");

    verify_that!(patterns.len(), eq(4))?;
    verify_that!(patterns, contains(eq("**/tests/**")))?;
    verify_that!(patterns, contains(eq("**/benches/**")))?;
    verify_that!(patterns, contains(eq("**/main.rs")))?;

    verify_that!(patterns, contains(eq("**/bin/**")))
}

#[gtest]
fn dedup_across_sets() -> Result<()> {
    let cfg = parse(
        r#"
[glob_sets]
a = ["**/tests/**", "**/benches/**"]
b = ["**/tests/**", "**/examples/**"]

[rules.panic]
enabled = true
ignore = ["a", "b"]
"#,
    );
    let patterns = cfg.ignore_patterns("panic");

    verify_that!(
        patterns.iter().filter(|p| *p == "**/tests/**").count(),
        eq(1)
    )?;
    verify_that!(patterns, contains(eq("**/benches/**")))?;

    verify_that!(patterns, contains(eq("**/examples/**")))
}

#[gtest]
fn literal_ignore_patterns_do_not_need_a_glob_set() -> Result<()> {
    let cfg = parse(
        r#"
[rules.panic]
enabled = true
ignore = ["generated/**", "src/generated.rs"]
"#,
    );

    verify_that!(cfg.ignore_patterns("panic"), contains(eq("generated/**")))?;

    verify_that!(
        cfg.ignore_patterns("panic"),
        contains(eq("src/generated.rs"))
    )
}

#[gtest]
fn explicit_unknown_glob_set_is_error() -> Result<()> {
    let raw: RawConfig = toml::from_str(
        r#"
[rules.panic]
enabled = true
ignore = ["@nonexistent"]
"#,
    )
    .or_fail()?;
    let err = Config::resolve(raw).unwrap_err();

    verify_that!(err, displays_as(contains_substring("unknown glob set")))
}

#[gtest]
fn malformed_literal_ignore_pattern_is_error() -> Result<()> {
    let raw: RawConfig = toml::from_str(
        r#"
[rules.panic]
enabled = true
ignore = ["src/[unterminated.rs"]
"#,
    )
    .or_fail()?;
    let error = Config::resolve(raw).unwrap_err();

    verify_that!(
        error,
        displays_as(all![
            contains_substring("invalid ignore pattern"),
            contains_substring("src/[unterminated.rs")
        ])
    )
}

#[gtest]
fn malformed_pattern_in_glob_set_is_error_even_before_use() -> Result<()> {
    let raw: RawConfig = toml::from_str(
        r#"
[glob_sets]
tests = ["src/[unterminated.rs"]

[rules.panic]
enabled = true
"#,
    )
    .or_fail()?;
    let error = Config::resolve(raw).unwrap_err();

    verify_that!(
        error,
        displays_as(all![
            contains_substring("glob set `tests`"),
            contains_substring("invalid pattern")
        ])
    )
}

#[gtest]
fn no_ignore_is_fine() -> Result<()> {
    let cfg = parse(
        r"
[rules.panic]
enabled = true
",
    );

    verify_that!(cfg.is_enabled("panic"), is_true())?;

    verify_that!(cfg.ignore_patterns("panic"), is_empty())
}

#[gtest]
fn validate_missing_rule_is_warning() -> Result<()> {
    let cfg = parse("[rules.panic]\nenabled = true\n");
    let (errors, warnings) = cfg.validate(&[("panic", &[]), ("style", &[])]);

    verify_that!(errors, is_empty())?;
    verify_that!(warnings.len(), eq(1))?;

    verify_that!(warnings[0], contains_substring("style"))
}

#[gtest]
fn validate_unknown_rule_is_warning() -> Result<()> {
    let cfg = parse("[rules.panic]\nenabled = true\n\n[rules.fake]\nenabled = true\n");
    let (errors, warnings) = cfg.validate(&[("panic", &[])]);

    verify_that!(errors, is_empty())?;
    verify_that!(warnings.len(), eq(1))?;

    verify_that!(warnings[0], contains_substring("fake"))
}

#[gtest]
fn validate_passes_when_complete() -> Result<()> {
    let cfg = parse("[rules.panic]\nenabled = true\n\n[rules.style]\nenabled = false\n");
    let (errors, warnings) = cfg.validate(&[("panic", &[]), ("style", &[])]);

    verify_that!(errors, is_empty())?;

    verify_that!(warnings, is_empty())
}

#[gtest]
fn validate_rejects_non_string_array_elements() -> Result<()> {
    const DEFAULTS: &[&str] = &[];
    const PARAMS: &[RuleParam] = &[RuleParam {
        name: "paths",
        param_type: ParamType::StringArray,
        default: ParamDefault::StringArray(DEFAULTS),
        allowed_values: &[],
    }];
    let cfg = parse("[rules.paths]\nenabled = true\npaths = [\"src/**\", 7]\n");

    let (errors, warnings) = cfg.validate(&[("paths", PARAMS)]);

    verify_that!(warnings, is_empty())?;

    verify_that!(
        errors,
        contains(contains_substring("must be an array of strings"))
    )
}

#[gtest]
fn validate_rejects_negative_integer_params() -> Result<()> {
    const PARAMS: &[RuleParam] = &[RuleParam {
        name: "limit",
        param_type: ParamType::Int,
        default: ParamDefault::Int(1),
        allowed_values: &[],
    }];
    let cfg = parse("[rules.size]\nenabled = true\nlimit = -1\n");
    let (errors, warnings) = cfg.validate(&[("size", PARAMS)]);

    verify_that!(warnings, is_empty())?;

    verify_that!(errors, contains(contains_substring("non-negative integer")))
}

#[gtest]
fn validate_rejects_unknown_or_duplicate_choice_values() -> Result<()> {
    const DEFAULTS: &[&str] = &["functions"];
    const ALLOWED: &[&str] = &["functions", "control-flow"];
    const PARAMS: &[RuleParam] = &[RuleParam {
        name: "boundaries",
        param_type: ParamType::StringArray,
        default: ParamDefault::StringArray(DEFAULTS),
        allowed_values: ALLOWED,
    }];
    let unknown =
        parse("[rules.padding]\nenabled = true\nboundaries = [\"functions\", \"function\"]\n");
    let duplicate =
        parse("[rules.padding]\nenabled = true\nboundaries = [\"functions\", \"functions\"]\n");

    let (unknown_errors, _) = unknown.validate(&[("padding", PARAMS)]);
    let (duplicate_errors, _) = duplicate.validate(&[("padding", PARAMS)]);

    verify_that!(
        unknown_errors,
        contains(contains_substring(
            "duplicate-free array of allowed strings"
        ))
    )?;

    verify_that!(
        duplicate_errors,
        contains(contains_substring(
            "duplicate-free array of allowed strings"
        ))
    )
}

#[gtest]
fn load_rejects_unknown_top_level_fields() -> Result<()> {
    let directory = Directory::new().or_fail()?;
    let path = directory.path().join("rulewright.toml");

    file::write_text(&path, "rulse = {}\n[rules.panic]\nenabled = true\n").or_fail()?;
    let error = Config::load(&path).unwrap_err();

    verify_that!(
        error,
        displays_as(contains_substring("unknown field `rulse`"))
    )
}

#[gtest]
fn generate_default_includes_all_rules() -> Result<()> {
    let cfg = Config::generate_default(&[("panic", &[]), ("style", &[]), ("dbg", &[])]);

    verify_that!(cfg.rules.len(), eq(3))?;

    verify_that!(cfg.is_enabled("panic"), is_true())
}

#[gtest]
fn generated_config_preserves_registry_defaults() -> Result<()> {
    let metadata = crate::RuleRegistry::with_builtins().or_fail()?.metadata();
    let cfg = Config::generate_registry_default(&metadata);

    verify_that!(cfg.rules.len(), eq(metadata.len()))?;

    for rule in metadata {
        verify_that!(cfg.is_enabled(rule.name), eq(rule.default_enabled))?;
    }

    Ok(())
}

#[gtest]
fn to_toml_produces_valid_toml_with_expected_rules() -> Result<()> {
    let cfg = Config::generate_default(&[("panic", &[]), ("style", &[])]);
    let toml_str = cfg.to_toml_string();
    let parsed: RawConfig = toml::from_str(&toml_str).or_fail()?;

    verify_that!(parsed.rules.len(), eq(2))?;
    verify_that!(parsed.rules.contains_key("panic"), is_true())?;
    verify_that!(parsed.rules.contains_key("style"), is_true())?;
    verify_that!(parsed.rules["panic"].enabled, is_true())?;

    verify_that!(toml_str, contains_substring("# rulewright.toml"))
}

#[gtest]
fn load_reports_a_missing_config_distinctly() -> Result<()> {
    let directory = Directory::new().or_fail()?;
    let path = directory.path().join("missing.toml");
    let error = Config::load(&path).unwrap_err();

    verify_that!(
        error,
        displays_as(contains_substring("file does not exist"))
    )?;

    verify_that!(
        error,
        displays_as(contains_substring(path.display().to_string()))
    )
}

#[gtest]
fn load_reports_non_missing_read_failures() -> Result<()> {
    let directory = Directory::new().or_fail()?;
    let path = directory.path().join("config.toml");

    directory::create(&path).or_fail()?;
    let error = Config::load(&path).unwrap_err();

    verify_that!(
        error,
        displays_as(contains_substring("failed to read file"))
    )?;

    verify_that!(
        error,
        displays_as(contains_substring(path.display().to_string()))
    )
}

#[gtest]
fn load_reports_parse_failures_with_the_config_path() -> Result<()> {
    let directory = Directory::new().or_fail()?;
    let path = directory.path().join("config.toml");

    file::write_text(&path, "[rules.invalid\n").or_fail()?;
    let error = Config::load(&path).unwrap_err();

    verify_that!(error, displays_as(contains_substring("failed to parse")))?;

    verify_that!(
        error,
        displays_as(contains_substring(path.display().to_string()))
    )
}
