// #rw(file: rust_inline_test_module_size) registry validation fixtures exercise private preflight paths

use std::collections::{BTreeMap, HashSet};

use crate::{AstCtx, FileCtx, Fix, TomlCtx, Violation, languages::workspace::WorkspaceCtx};

/// A rule entry enriched with resolved configuration for machine-readable reporting.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ConfigRule {
    pub name: String,
    pub description: String,
    pub severity: String,
    pub category: String,
    pub fixable: bool,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignore: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<BTreeMap<String, ConfigValue>>,
}

/// JSON-compatible resolved configuration value.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
pub struct ConfigValue(serde_json::Value);

impl From<serde_json::Value> for ConfigValue {
    fn from(value: serde_json::Value) -> Self {
        Self(value)
    }
}

impl PartialEq<i32> for ConfigValue {
    fn eq(&self, other: &i32) -> bool {
        self.0.as_i64() == Some(i64::from(*other))
    }
}

/// The language and analysis context a rule operates on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
// #rw(rust_non_exhaustive_on_public) internal enum, all variants matched within this crate
pub enum RuleKind {
    RustLine,
    RustAst,
    RustWorkspace,
    Workspace,
    Toml,
}

impl RuleKind {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleKind::RustLine => "rust-line",
            RuleKind::RustAst => "rust-ast",
            RuleKind::RustWorkspace => "rust-workspace",
            RuleKind::Workspace => "workspace",
            RuleKind::Toml => "toml",
        }
    }
}

impl std::fmt::Display for RuleKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Unified metadata for a registered rule (line or AST).
#[derive(Debug)]
pub struct RuleMeta {
    pub name: &'static str,
    pub description: &'static str,
    pub justification: &'static str,
    pub severity: Severity,
    pub kind: RuleKind,
    pub examples: &'static [Example],
    pub params: &'static [RuleParam],
    pub fixable: bool,
    pub default_enabled: bool,
}

/// Collect all registered rules sorted by name.
#[must_use]
#[doc(alias = "lint registry", alias = "rulewright rules")]
pub fn all_rules() -> Vec<RuleMeta> {
    let mut rules: Vec<RuleMeta> = inventory::iter::<Rule>
        .into_iter()
        .map(|rule| rule.info.to_meta(rule.check.kind(), rule.fix.is_some()))
        .collect();

    rules.sort_by_key(|r| r.name);

    rules
}

/// A statically linked downstream rule pack.
#[derive(Clone, Copy, Debug)]
pub struct RulePack {
    /// Stable pack identifier used in diagnostics and cache identity.
    pub name: &'static str,
    /// Pack version used in cache identity.
    pub version: &'static str,
    /// Caller-controlled implementation fingerprint for embedded use.
    pub implementation_fingerprint: &'static str,
    /// Rules contributed by this pack.
    pub rules: &'static [Rule],
}

/// A deterministic collection of built-in and downstream rules.
#[derive(Debug, Default)]
pub struct RuleRegistry {
    rules: Vec<&'static Rule>,
    packs: Vec<RulePack>,
}

impl RuleRegistry {
    /// Create an empty registry.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            rules: Vec::new(),
            packs: Vec::new(),
        }
    }

    /// Create a registry containing Rulewright's generic built-in rules.
    ///
    /// # Errors
    ///
    /// Returns a duplicate-ID error if the compiled built-in inventory is invalid.
    pub fn with_builtins() -> Result<Self, RegistryError> {
        let mut registry = Self::new();
        let mut rules: Vec<&'static Rule> = inventory::iter::<Rule>.into_iter().collect();

        rules.sort_by_key(|rule| rule.info.name);
        registry.insert_rules("rulewright", &rules)?;

        Ok(registry)
    }

    /// Add one statically linked downstream pack.
    ///
    /// # Errors
    ///
    /// Returns a duplicate-ID error without partially registering the pack.
    pub fn extend(&mut self, pack: RulePack) -> Result<(), RegistryError> {
        if !valid_pack_name(pack.name) {
            return Err(RegistryError::InvalidPackName);
        }

        if pack.name == "rulewright" {
            return Err(RegistryError::ReservedPackName);
        }

        if self
            .packs
            .iter()
            .any(|registered| registered.name == pack.name)
        {
            return Err(RegistryError::DuplicatePack(pack.name));
        }

        if semver::Version::parse(pack.version).is_err() {
            return Err(RegistryError::InvalidPackVersion {
                pack: pack.name,
                version: pack.version,
            });
        }

        if pack.implementation_fingerprint.trim().is_empty() {
            return Err(RegistryError::MissingFingerprint(pack.name));
        }

        let rules: Vec<&'static Rule> = pack.rules.iter().collect();

        self.insert_rules(pack.name, &rules)?;
        self.packs.push(pack);
        self.packs.sort_by_key(|registered| registered.name);

        Ok(())
    }

    /// Return registered rules in stable ID order.
    #[must_use]
    pub fn rules(&self) -> &[&'static Rule] {
        &self.rules
    }

    /// Return registered downstream packs in stable name order.
    #[must_use]
    pub fn packs(&self) -> &[RulePack] {
        &self.packs
    }

    /// Return public metadata for every registered rule.
    #[must_use]
    pub fn metadata(&self) -> Vec<RuleMeta> {
        self.rules
            .iter()
            .map(|rule| rule.info.to_meta(rule.check.kind(), rule.fix.is_some()))
            .collect()
    }

    fn insert_rules(
        &mut self,
        pack_name: &'static str,
        rules: &[&'static Rule],
    ) -> Result<(), RegistryError> {
        let mut known: HashSet<&str> = self.rules.iter().map(|rule| rule.info.name).collect();

        for rule in rules {
            validate_rule(pack_name, rule)?;

            if !known.insert(rule.info.name) {
                return Err(RegistryError::DuplicateRule {
                    pack: pack_name,
                    rule: rule.info.name,
                });
            }
        }

        self.rules.extend_from_slice(rules);
        self.rules.sort_by_key(|rule| rule.info.name);

        Ok(())
    }
}

/// Failure to construct a deterministic rule registry.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error(
        "rule pack name must start with a lowercase letter and contain only lowercase letters, digits, `-`, or `_`"
    )]
    InvalidPackName,
    #[error("rule pack name `rulewright` is reserved for built-in rules")]
    ReservedPackName,
    #[error("rule pack `{0}` is already registered")]
    DuplicatePack(&'static str),
    #[error("rule pack `{pack}` has invalid semantic version `{version}`")]
    InvalidPackVersion {
        pack: &'static str,
        version: &'static str,
    },
    #[error("rule pack `{0}` must provide an implementation fingerprint")]
    MissingFingerprint(&'static str),
    #[error("rule pack `{pack}` registers duplicate rule ID `{rule}`")]
    DuplicateRule {
        pack: &'static str,
        rule: &'static str,
    },
    #[error("rule pack `{pack}` has invalid rule `{rule}`: {reason}")]
    InvalidRule {
        pack: &'static str,
        rule: &'static str,
        reason: &'static str,
    },
}

/// A code example that demonstrates a rule — used in `--detail` output and tests.
#[derive(Clone, Copy, Debug)]
pub struct Example {
    pub label: &'static str,
    pub code: &'static str,
    pub pass: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
// #rw(rust_non_exhaustive_on_public) internal enum, all variants matched within this crate
pub enum ParamType {
    Int,
    StringArray,
}

impl ParamType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Int => "i64",
            Self::StringArray => "string-array",
        }
    }
}

#[derive(Clone, Copy, Debug)]
// #rw(rust_non_exhaustive_on_public) internal enum, all variants matched within this crate
pub enum ParamDefault {
    Int(i64),
    StringArray(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug)]
pub struct RuleParam {
    pub name: &'static str,
    pub param_type: ParamType,
    pub default: ParamDefault,
    pub allowed_values: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
// #rw(rust_non_exhaustive_on_public) severity levels are a fixed set
pub enum Severity {
    Low,
    Medium,
    High,
}

impl Severity {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Shared metadata fields common to both line and AST rules.
#[derive(Clone, Copy, Debug)]
pub struct RuleInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub justification: &'static str,
    pub severity: Severity,
    pub examples: &'static [Example],
    pub params: &'static [RuleParam],
    pub default_enabled: bool,
}

impl RuleInfo {
    /// Construct rule metadata for a statically linked rule.
    #[must_use]
    pub const fn new(
        name: &'static str,
        description: &'static str,
        justification: &'static str,
        severity: Severity,
        examples: &'static [Example],
        params: &'static [RuleParam],
    ) -> Self {
        Self {
            name,
            description,
            justification,
            severity,
            examples,
            params,
            default_enabled: true,
        }
    }

    /// Keep the rule available while leaving it disabled in generated configurations.
    #[must_use]
    pub const fn disabled_by_default(mut self) -> Self {
        self.default_enabled = false;

        self
    }

    fn to_meta(self, kind: RuleKind, fixable: bool) -> RuleMeta {
        RuleMeta {
            name: self.name,
            description: self.description,
            justification: self.justification,
            severity: self.severity,
            kind,
            examples: self.examples,
            params: self.params,
            fixable,
            default_enabled: self.default_enabled,
        }
    }
}

fn valid_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();

    bytes.next().is_some_and(|first| first.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_pack_name(value: &str) -> bool {
    let mut bytes = value.bytes();

    bytes.next().is_some_and(|first| first.is_ascii_lowercase())
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

fn validate_rule(pack: &'static str, rule: &Rule) -> Result<(), RegistryError> {
    let invalid = |reason| RegistryError::InvalidRule {
        pack,
        rule: rule.info.name,
        reason,
    };

    if !valid_identifier(rule.info.name) {
        return Err(invalid(
            "ID must start with a lowercase letter and contain only lowercase letters, digits, or `_`",
        ));
    }

    if rule.info.description.trim().is_empty() || rule.info.justification.trim().is_empty() {
        return Err(invalid("description and justification must not be empty"));
    }

    if rule
        .info
        .examples
        .iter()
        .any(|example| example.label.trim().is_empty() || example.code.trim().is_empty())
    {
        return Err(invalid("example labels and source must not be empty"));
    }

    let mut params = HashSet::new();

    for parameter in rule.info.params {
        if !valid_identifier(parameter.name) {
            return Err(invalid(
                "parameter names must be valid lowercase identifiers",
            ));
        }

        if !params.insert(parameter.name) {
            return Err(invalid("parameter names must be unique"));
        }

        match (&parameter.param_type, &parameter.default) {
            (ParamType::Int, ParamDefault::Int(value)) if *value < 0 => {
                return Err(invalid("integer parameter defaults must be non-negative"));
            }

            (ParamType::Int, ParamDefault::Int(_)) => {}

            (ParamType::StringArray, ParamDefault::StringArray(defaults)) => {
                let mut unique_defaults = HashSet::new();

                if defaults.iter().any(|value| !unique_defaults.insert(*value)) {
                    return Err(invalid(
                        "string-array parameter defaults must not contain duplicates",
                    ));
                }

                if !parameter.allowed_values.is_empty()
                    && defaults
                        .iter()
                        .any(|value| !parameter.allowed_values.contains(value))
                {
                    return Err(invalid("string-array defaults must use allowed values"));
                }
            }

            _ => return Err(invalid("parameter type does not match its default value")),
        }
    }

    let compatible = match rule.check {
        RuleCheck::RustLine(_) | RuleCheck::RustLineFull(_) => {
            matches!(rule.fix, None | Some(RuleFix::RustLine(_)))
        }

        RuleCheck::RustAst(_) => {
            matches!(
                rule.fix,
                None | Some(RuleFix::RustAst(_) | RuleFix::RustAstTree(_))
            )
        }

        RuleCheck::RustWorkspace(_) | RuleCheck::Workspace(_) => rule.fix.is_none(),

        RuleCheck::Toml(_) => matches!(rule.fix, None | Some(RuleFix::Toml(_))),
    };

    if !compatible {
        return Err(invalid("fix kind does not match check kind"));
    }

    Ok(())
}

/// Language-specific rule check function stored in the unified registry.
#[derive(Clone, Copy, Debug)]
pub enum RuleCheck {
    RustLine(fn(&FileCtx<'_>) -> Vec<Violation>),
    RustLineFull(fn(&FileCtx<'_>) -> Vec<Violation>),
    RustAst(fn(&AstCtx<'_>) -> Vec<Violation>),
    RustWorkspace(fn(&WorkspaceCtx<'_>) -> Vec<Violation>),
    Workspace(fn(&WorkspaceCtx<'_>) -> Vec<Violation>),
    Toml(fn(&TomlCtx<'_>) -> Vec<Violation>),
}

impl RuleCheck {
    #[must_use]
    pub const fn kind(self) -> RuleKind {
        match self {
            Self::RustLine(_) | Self::RustLineFull(_) => RuleKind::RustLine,
            Self::RustAst(_) => RuleKind::RustAst,
            Self::RustWorkspace(_) => RuleKind::RustWorkspace,
            Self::Workspace(_) => RuleKind::Workspace,
            Self::Toml(_) => RuleKind::Toml,
        }
    }

    pub(crate) const fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::RustLine(_)
            | Self::RustLineFull(_)
            | Self::RustAst(_)
            | Self::RustWorkspace(_) => &["rs"],
            Self::Workspace(_) => &["rs", "toml"],
            Self::Toml(_) => &["toml"],
        }
    }
}

/// Language-specific fix function corresponding to [`RuleCheck`].
#[derive(Clone, Copy, Debug)]
pub enum RuleFix {
    RustLine(fn(&FileCtx<'_>, &Violation) -> Option<Fix>),
    RustAst(fn(&AstCtx<'_>, &Violation) -> Option<Fix>),
    RustAstTree(fn(&AstCtx<'_>, &[Violation]) -> Option<String>),
    Toml(fn(&TomlCtx<'_>, &Violation) -> Option<Fix>),
}

/// One lint rule in the language-neutral registry.
#[derive(Clone, Copy, Debug)]
pub struct Rule {
    pub info: RuleInfo,
    pub check: RuleCheck,
    pub fix: Option<RuleFix>,
}

impl Rule {
    /// Construct a Rust line rule.
    #[must_use]
    pub const fn rust_line(
        info: RuleInfo,
        check: fn(&FileCtx<'_>) -> Vec<Violation>,
        fix: Option<fn(&FileCtx<'_>, &Violation) -> Option<Fix>>,
    ) -> Self {
        Self {
            info,
            check: RuleCheck::RustLine(check),
            fix: match fix {
                Some(fix) => Some(RuleFix::RustLine(fix)),
                None => None,
            },
        }
    }

    /// Construct a Rust line rule that sees complete source, including test-only items.
    ///
    /// The check must distinguish code from marker lookalikes in strings or comments.
    /// Prefer [`Self::rust_line`] unless the rule is explicitly opted into within source.
    #[must_use]
    pub const fn rust_line_full(
        info: RuleInfo,
        check: fn(&FileCtx<'_>) -> Vec<Violation>,
        fix: Option<fn(&FileCtx<'_>, &Violation) -> Option<Fix>>,
    ) -> Self {
        Self {
            info,
            check: RuleCheck::RustLineFull(check),
            fix: match fix {
                Some(fix) => Some(RuleFix::RustLine(fix)),
                None => None,
            },
        }
    }

    /// Construct a Rust AST rule.
    #[must_use]
    pub const fn rust_ast(
        info: RuleInfo,
        check: fn(&AstCtx<'_>) -> Vec<Violation>,
        fix: Option<fn(&AstCtx<'_>, &Violation) -> Option<Fix>>,
    ) -> Self {
        Self {
            info,
            check: RuleCheck::RustAst(check),
            fix: match fix {
                Some(fix) => Some(RuleFix::RustAst(fix)),
                None => None,
            },
        }
    }

    /// Construct a coordinated Rust AST-tree fix rule.
    #[must_use]
    pub const fn rust_ast_tree(
        info: RuleInfo,
        check: fn(&AstCtx<'_>) -> Vec<Violation>,
        fix: fn(&AstCtx<'_>, &[Violation]) -> Option<String>,
    ) -> Self {
        Self {
            info,
            check: RuleCheck::RustAst(check),
            fix: Some(RuleFix::RustAstTree(fix)),
        }
    }

    /// Construct a Rust workspace rule.
    #[must_use]
    pub const fn rust_workspace(
        info: RuleInfo,
        check: fn(&WorkspaceCtx<'_>) -> Vec<Violation>,
    ) -> Self {
        Self {
            info,
            check: RuleCheck::RustWorkspace(check),
            fix: None,
        }
    }

    /// Construct a language-neutral workspace rule.
    #[must_use]
    pub const fn workspace(info: RuleInfo, check: fn(&WorkspaceCtx<'_>) -> Vec<Violation>) -> Self {
        Self {
            info,
            check: RuleCheck::Workspace(check),
            fix: None,
        }
    }

    /// Construct a TOML rule.
    #[must_use]
    pub const fn toml(
        info: RuleInfo,
        check: fn(&TomlCtx<'_>) -> Vec<Violation>,
        fix: Option<fn(&TomlCtx<'_>, &Violation) -> Option<Fix>>,
    ) -> Self {
        Self {
            info,
            check: RuleCheck::Toml(check),
            fix: match fix {
                Some(fix) => Some(RuleFix::Toml(fix)),
                None => None,
            },
        }
    }

    /// Return this rule's stable identifier.
    #[must_use]
    pub const fn id(&self) -> &'static str {
        self.info.name
    }
}

inventory::collect!(Rule);

#[cfg(test)]
mod tests {
    use super::*;

    fn no_violations(_: &FileCtx<'_>) -> Vec<Violation> {
        Vec::new()
    }

    fn no_toml_fix(_: &TomlCtx<'_>, _: &Violation) -> Option<Fix> {
        None
    }

    const DUPLICATE_PARAMS: &[RuleParam] = &[
        RuleParam {
            name: "limit",
            param_type: ParamType::Int,
            default: ParamDefault::Int(1),
            allowed_values: &[],
        },
        RuleParam {
            name: "limit",
            param_type: ParamType::Int,
            default: ParamDefault::Int(2),
            allowed_values: &[],
        },
    ];
    const INVALID_NAME_PARAMS: &[RuleParam] = &[RuleParam {
        name: "Invalid-Param",
        param_type: ParamType::Int,
        default: ParamDefault::Int(1),
        allowed_values: &[],
    }];
    const NEGATIVE_DEFAULT_PARAMS: &[RuleParam] = &[RuleParam {
        name: "limit",
        param_type: ParamType::Int,
        default: ParamDefault::Int(-1),
        allowed_values: &[],
    }];
    const INT_WITH_ARRAY_DEFAULT: &[RuleParam] = &[RuleParam {
        name: "limit",
        param_type: ParamType::Int,
        default: ParamDefault::StringArray(&[]),
        allowed_values: &[],
    }];
    const ARRAY_WITH_INT_DEFAULT: &[RuleParam] = &[RuleParam {
        name: "names",
        param_type: ParamType::StringArray,
        default: ParamDefault::Int(1),
        allowed_values: &[],
    }];
    const DUPLICATE_ARRAY_DEFAULT_PARAMS: &[RuleParam] = &[RuleParam {
        name: "names",
        param_type: ParamType::StringArray,
        default: ParamDefault::StringArray(&["duplicate", "duplicate"]),
        allowed_values: &[],
    }];
    const EMPTY_EXAMPLES: &[Example] = &[Example {
        label: "",
        code: "fn fixture() {}",
        pass: true,
    }];

    static DUPLICATE_RULES: &[Rule] = &[Rule::rust_line(
        RuleInfo::new(
            "rust_dbg",
            "Duplicate fixture.",
            "Duplicate IDs must be rejected before the registry changes.",
            Severity::Low,
            &[],
            &[],
        ),
        no_violations,
        None,
    )];

    static INVALID_ID_RULES: &[Rule] = &[Rule::rust_line(
        RuleInfo::new(
            "Invalid-Rule",
            "Invalid fixture.",
            "Invalid IDs must be rejected.",
            Severity::Low,
            &[],
            &[],
        ),
        no_violations,
        None,
    )];

    static EMPTY_METADATA_RULES: &[Rule] = &[Rule::rust_line(
        RuleInfo::new(
            "fixture_empty_metadata",
            "",
            "Missing descriptions must be rejected.",
            Severity::Low,
            &[],
            &[],
        ),
        no_violations,
        None,
    )];

    static DUPLICATE_PARAM_RULES: &[Rule] = &[Rule::rust_line(
        RuleInfo::new(
            "fixture_duplicate_params",
            "Duplicate parameter fixture.",
            "Duplicate parameter names make configuration ambiguous.",
            Severity::Low,
            &[],
            DUPLICATE_PARAMS,
        ),
        no_violations,
        None,
    )];

    static INCOMPATIBLE_FIX_RULES: &[Rule] = &[Rule {
        info: RuleInfo::new(
            "fixture_incompatible_fix",
            "Incompatible fix fixture.",
            "A TOML fix cannot edit a Rust line-rule violation.",
            Severity::Low,
            &[],
            &[],
        ),
        check: RuleCheck::RustLine(no_violations),
        fix: Some(RuleFix::Toml(no_toml_fix)),
    }];

    static INVALID_PARAM_NAME_RULES: &[Rule] = &[Rule::rust_line(
        RuleInfo::new(
            "fixture_invalid_param_name",
            "Invalid parameter fixture.",
            "Parameter names form stable configuration keys.",
            Severity::Low,
            &[],
            INVALID_NAME_PARAMS,
        ),
        no_violations,
        None,
    )];

    static NEGATIVE_DEFAULT_RULES: &[Rule] = &[Rule::rust_line(
        RuleInfo::new(
            "fixture_negative_default",
            "Negative default fixture.",
            "Unsigned threshold accessors cannot represent negative defaults.",
            Severity::Low,
            &[],
            NEGATIVE_DEFAULT_PARAMS,
        ),
        no_violations,
        None,
    )];

    static INT_WITH_ARRAY_DEFAULT_RULES: &[Rule] = &[Rule::rust_line(
        RuleInfo::new(
            "fixture_int_with_array_default",
            "Mismatched parameter fixture.",
            "Parameter types and defaults must agree.",
            Severity::Low,
            &[],
            INT_WITH_ARRAY_DEFAULT,
        ),
        no_violations,
        None,
    )];

    static ARRAY_WITH_INT_DEFAULT_RULES: &[Rule] = &[Rule::rust_line(
        RuleInfo::new(
            "fixture_array_with_int_default",
            "Mismatched parameter fixture.",
            "Parameter types and defaults must agree.",
            Severity::Low,
            &[],
            ARRAY_WITH_INT_DEFAULT,
        ),
        no_violations,
        None,
    )];

    static DUPLICATE_ARRAY_DEFAULT_RULES: &[Rule] = &[Rule::rust_line(
        RuleInfo::new(
            "fixture_duplicate_array_default",
            "Duplicate array default fixture.",
            "Generated configuration must not contain invalid duplicate values.",
            Severity::Low,
            &[],
            DUPLICATE_ARRAY_DEFAULT_PARAMS,
        ),
        no_violations,
        None,
    )];

    static EMPTY_EXAMPLE_RULES: &[Rule] = &[Rule::rust_line(
        RuleInfo::new(
            "fixture_empty_example",
            "Empty example fixture.",
            "Examples need labels and source to be useful.",
            Severity::Low,
            EMPTY_EXAMPLES,
            &[],
        ),
        no_violations,
        None,
    )];

    fn fixture_pack(name: &'static str, rules: &'static [Rule]) -> RulePack {
        RulePack {
            name,
            version: "1.0.0",
            implementation_fingerprint: "fixture:v1",
            rules,
        }
    }

    #[test]
    fn builtin_inventory_is_sorted_and_spans_every_rule_kind() {
        let metadata = RuleRegistry::with_builtins()
            .expect("built-in rule IDs should be unique")
            .metadata();

        assert!(metadata.iter().any(|rule| rule.fixable));

        for kind in [
            RuleKind::RustLine,
            RuleKind::RustAst,
            RuleKind::RustWorkspace,
            RuleKind::Workspace,
            RuleKind::Toml,
        ] {
            assert!(metadata.iter().any(|rule| rule.kind == kind), "{kind:?}");
        }

        assert!(metadata.windows(2).all(|pair| pair[0].name < pair[1].name));
        assert!(
            metadata
                .iter()
                .all(|rule| !rule.justification.contains("M-"))
        );
        assert!(
            metadata
                .iter()
                .find(|rule| rule.name == "rust_mutex_in_async")
                .is_some_and(|rule| !rule.default_enabled)
        );
    }

    #[test]
    fn duplicate_external_rule_is_rejected_without_partial_registration() {
        let mut registry =
            RuleRegistry::with_builtins().expect("built-in rule IDs should be unique");
        let original_rules = registry.rules().len();
        let error = registry
            .extend(RulePack {
                name: "duplicate-fixture",
                version: "1.0.0",
                implementation_fingerprint: "duplicate-fixture:v1",
                rules: DUPLICATE_RULES,
            })
            .expect_err("duplicate ID should be rejected");

        assert_eq!(
            error.to_string(),
            "rule pack `duplicate-fixture` registers duplicate rule ID `rust_dbg`"
        );
        assert_eq!(registry.rules().len(), original_rules);
        assert!(registry.packs().is_empty());
    }

    #[test]
    fn invalid_pack_identity_is_rejected() {
        let mut registry = RuleRegistry::new();

        assert!(matches!(
            registry.extend(fixture_pack("Bad Pack", &[])),
            Err(RegistryError::InvalidPackName)
        ));
        assert!(matches!(
            registry.extend(fixture_pack("rulewright", &[])),
            Err(RegistryError::ReservedPackName)
        ));
        assert!(registry.rules().is_empty());
        assert!(registry.packs().is_empty());
    }

    #[test]
    fn malformed_rule_metadata_is_rejected_without_partial_registration() {
        #[rustfmt::skip]
        let cases = [
            // #rw:aligned
            (INVALID_ID_RULES,              "ID must start"),
            (EMPTY_METADATA_RULES,          "must not be empty"),
            (DUPLICATE_PARAM_RULES,         "parameter names must be unique"),
            (INVALID_PARAM_NAME_RULES,      "parameter names must be valid lowercase identifiers"),
            (NEGATIVE_DEFAULT_RULES,        "integer parameter defaults must be non-negative"),
            (INT_WITH_ARRAY_DEFAULT_RULES,  "parameter type does not match"),
            (ARRAY_WITH_INT_DEFAULT_RULES,  "parameter type does not match"),
            (DUPLICATE_ARRAY_DEFAULT_RULES, "string-array parameter defaults must not contain duplicates"),
            (EMPTY_EXAMPLE_RULES,           "example labels and source"),
            (INCOMPATIBLE_FIX_RULES,        "fix kind does not match"),
        ];

        for (rules, expected) in cases {
            let mut registry = RuleRegistry::new();
            let error = registry
                .extend(fixture_pack("fixture-pack", rules))
                .expect_err("invalid rule should be rejected");

            assert!(error.to_string().contains(expected), "{error}");
            assert!(registry.rules().is_empty());
            assert!(registry.packs().is_empty());
        }
    }
}
