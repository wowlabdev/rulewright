// #rw(file: rust_default_hasher, rust_missing_capacity, rust_vec_string_field) cold config-load path; hashing, preallocation, and boxed slices are not bottlenecks

use std::collections::{BTreeMap, HashSet};

use crate::{
    file,
    path::{Path, PathBuf},
};
use serde::{Deserialize, Serialize};

use crate::{ParamDefault, ParamType, RuleParam};

const fn enabled_by_default() -> bool {
    true
}

#[derive(Debug, Deserialize, Serialize)]
struct RawRuleConfig {
    enabled: bool,
    #[serde(default)]
    ignore: Vec<String>,
    #[serde(default)]
    only: Vec<String>,
    #[serde(default)]
    overrides: Vec<RuleOverride>,
    #[serde(default, flatten)]
    params: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default = "enabled_by_default")]
    allow_suppressions: bool,
    #[serde(default)]
    glob_sets: BTreeMap<String, Vec<String>>,
    rules: BTreeMap<String, RawRuleConfig>,
}

/// Resolved rule configuration (glob set names expanded to patterns).
#[derive(Clone, Debug, Serialize)]
pub(crate) struct RuleConfig {
    pub(crate) enabled: bool,
    pub(crate) ignore: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) only: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) overrides: Vec<RuleOverride>,
    #[serde(default, flatten)]
    pub(crate) params: BTreeMap<String, toml::Value>,
}

/// Path-scoped parameter values; matching entries apply in declaration order, last value wins.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RuleOverride {
    /// Workspace-relative globs, with named glob sets expanded after loading.
    pub paths: Vec<String>,
    /// Typed rule parameters overridden on matching paths.
    #[serde(flatten)]
    pub params: BTreeMap<String, toml::Value>,
}

/// Top-level rulewright configuration loaded from `rulewright.toml`.
#[derive(Debug, Serialize)]
pub struct Config {
    allow_suppressions: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    glob_sets: BTreeMap<String, Vec<String>>,
    pub(crate) rules: BTreeMap<String, RuleConfig>,
    #[serde(skip)]
    workspace: super::workspace::WorkspaceContext,
}

/// Failure to load or resolve rulewright configuration.
#[derive(Debug, thiserror::Error)]
#[error("{kind}")]
pub struct ConfigError {
    #[source]
    kind: ConfigErrorKind,
}

#[derive(Debug, thiserror::Error)]
enum ConfigErrorKind {
    #[error("failed to read {}: file does not exist", path.display())]
    Missing { path: PathBuf },
    #[error("{source}")]
    Read {
        #[source]
        source: crate::error::Error,
    },
    #[error("failed to parse {}: {source}", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("{message}")]
    Resolve { message: String },
}

impl ConfigError {
    const fn new(kind: ConfigErrorKind) -> Self {
        Self { kind }
    }
}

impl Config {
    /// Load, parse, and resolve a config file (expands glob set references).
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read, parsed, or resolved against its glob sets.
    pub fn load(path: &Path) -> Result<Config, ConfigError> {
        let contents = file::read_text_if_exists(path)
            .map_err(|source| ConfigError::new(ConfigErrorKind::Read { source }))?
            .ok_or_else(|| {
                ConfigError::new(ConfigErrorKind::Missing {
                    path: path.to_path_buf(),
                })
            })?;
        let raw: RawConfig = toml::from_str(&contents).map_err(|source| {
            ConfigError::new(ConfigErrorKind::Parse {
                path: path.to_path_buf(),
                source,
            })
        })?;

        Self::resolve(raw)
    }

    pub fn is_enabled(&self, name: &str) -> bool {
        self.rules.get(name).is_some_and(|r| r.enabled)
    }

    /// Whether source-level `#rw(...)` suppression directives are permitted.
    #[must_use]
    pub const fn allows_suppressions(&self) -> bool {
        self.allow_suppressions
    }

    pub fn ignore_patterns(&self, name: &str) -> &[String] {
        self.rules.get(name).map_or(&[], |r| &r.ignore)
    }

    /// Whether a path is in the rule's positive scope and outside its ignores.
    #[must_use]
    pub fn applies_to(&self, name: &str, rel: &str) -> bool {
        self.rules.get(name).is_none_or(|rule| {
            (rule.only.is_empty() || crate::matches_ignore(rel, &rule.only))
                && !crate::matches_ignore(rel, &rule.ignore)
        })
    }

    fn parameter(&self, rule: &str, param: &RuleParam, rel: &str) -> Option<&toml::Value> {
        let rule = self.rules.get(rule)?;

        rule.overrides
            .iter()
            .rev()
            .filter(|entry| crate::matches_ignore(rel, &entry.paths))
            .find_map(|entry| entry.params.get(param.name))
            .or_else(|| rule.params.get(param.name))
    }

    // #rw(rust_getter_prefix) keyed param lookup mirrors the toml::Value getter family
    pub fn get_str_array(&self, rule: &str, param: &RuleParam, rel: &str) -> Vec<String> {
        let configured = self.parameter(rule, param, rel).and_then(|v| v.as_array());

        configured.map_or_else(
            || match &param.default {
                ParamDefault::StringArray(d) => d.iter().map(ToString::to_string).collect(),

                ParamDefault::Int(_) | ParamDefault::Bool(_) => {
                    unreachable!("get_str_array is only called for StringArray params")
                }
            },
            |arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            },
        )
    }

    /// Validate config against registered rules, returning `(errors, warnings)`.
    pub fn validate(
        &self,
        registered: &[(&str, &'static [RuleParam])],
    ) -> (Vec<String>, Vec<String>) {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let registered_set: HashSet<&str> = registered.iter().map(|(name, _)| *name).collect();

        for (name, params) in registered {
            let Some(rule_cfg) = self.rules.get(*name) else {
                warnings.push(format!(
                    "rule `{name}` is registered but missing from rulewright.toml — \
                     using defaults (run `rulewright --init` to regenerate)"
                ));
                continue;
            };

            for p in *params {
                Self::validate_param(name, p, rule_cfg, &mut errors);
            }

            for entry in &rule_cfg.overrides {
                let mut merged = rule_cfg.clone();

                merged.params.extend(entry.params.clone());

                for p in *params {
                    Self::validate_param(name, p, &merged, &mut errors);
                }

                for key in entry.params.keys() {
                    if !params.iter().any(|param| param.name == key) {
                        errors.push(format!("rule `{name}` override has unknown param `{key}`"));
                    }
                }
            }

            let known: HashSet<&str> = params.iter().map(|p| p.name).collect();

            for key in rule_cfg.params.keys() {
                if !known.contains(key.as_str()) {
                    errors.push(format!(
                        "rule `{name}` has unknown param `{key}` in rulewright.toml — \
                         remove it or check for typos"
                    ));
                }
            }
        }

        for name in self.rules.keys() {
            if !registered_set.contains(name.as_str()) {
                warnings.push(format!(
                    "rulewright.toml contains unknown rule `{name}` — \
                     remove it or check for typos"
                ));
            }
        }

        (errors, warnings)
    }

    pub fn backfill_defaults(&mut self, registered: &[(&str, &'static [RuleParam])]) {
        for (name, params) in registered {
            if !self.rules.contains_key(*name) {
                let mut param_map = BTreeMap::new();

                for p in *params {
                    let val = match &p.default {
                        ParamDefault::Int(d) => toml::Value::Integer(*d),

                        ParamDefault::Bool(d) => toml::Value::Boolean(*d),

                        ParamDefault::StringArray(d) => toml::Value::Array(
                            d.iter()
                                .map(|s| toml::Value::String(s.to_string()))
                                .collect(),
                        ),
                    };

                    param_map.insert(p.name.to_string(), val);
                }

                self.rules.insert(
                    name.to_string(),
                    RuleConfig {
                        enabled: true,
                        ignore: Vec::new(),
                        only: Vec::new(),
                        overrides: Vec::new(),
                        params: param_map,
                    },
                );
            }
        }
    }

    pub(crate) fn backfill_registry_defaults(&mut self, registered: &[crate::RuleMeta]) {
        for rule in registered {
            self.insert_default(rule.name, rule.params, rule.default_enabled);
        }
    }

    #[must_use]
    pub fn generate_default(registered: &[(&str, &'static [RuleParam])]) -> Config {
        let mut rules = BTreeMap::new();

        for (name, params) in registered {
            let mut param_map = BTreeMap::new();

            for p in *params {
                let val = match &p.default {
                    ParamDefault::Int(d) => toml::Value::Integer(*d),

                    ParamDefault::Bool(d) => toml::Value::Boolean(*d),

                    ParamDefault::StringArray(d) => toml::Value::Array(
                        d.iter()
                            .map(|s| toml::Value::String(s.to_string()))
                            .collect(),
                    ),
                };

                param_map.insert(p.name.to_string(), val);
            }

            rules.insert(
                name.to_string(),
                RuleConfig {
                    enabled: true,
                    ignore: Vec::new(),
                    only: Vec::new(),
                    overrides: Vec::new(),
                    params: param_map,
                },
            );
        }

        Config {
            allow_suppressions: true,
            glob_sets: BTreeMap::new(),
            rules,
            workspace: super::workspace::WorkspaceContext::default(),
        }
    }

    pub(crate) fn generate_registry_default(registered: &[crate::RuleMeta]) -> Config {
        let mut config = Config {
            allow_suppressions: true,
            glob_sets: BTreeMap::new(),
            rules: BTreeMap::new(),
            workspace: super::workspace::WorkspaceContext::default(),
        };

        for rule in registered {
            config.insert_default(rule.name, rule.params, rule.default_enabled);
        }

        config
    }

    /// Serializes this configuration with the standard explanatory header.
    ///
    /// # Panics
    ///
    /// Panics if the resolved configuration cannot be serialized as TOML.
    pub fn to_toml_string(&self) -> String {
        let header = "\
# rulewright.toml — lint rule configuration.
#
# Every registered rule MUST appear here. Omitting a rule is an error.
# Set `enabled = false` to disable a rule without removing its config.
# Set `allow_suppressions = false` to reject every source-level #rw(...) directive.
# Run `rulewright --init` to generate a fresh config with all rules.
#
# [glob_sets] defines named pattern collections.
# Rules can use literal patterns or reference a named set. Prefixing a set with `@` is explicit.

";
        let body = toml::to_string_pretty(self).expect("config is serializable");

        format!("{header}{body}")
    }

    /// Merge resolved configuration with registered rule metadata.
    pub fn resolved_rules(&self, rules: &[crate::RuleMeta]) -> Vec<crate::ConfigRule> {
        rules
            .iter()
            .map(|rule| {
                let configured = self.rules.get(rule.name);
                let non_empty_params = configured
                    .map(|config| &config.params)
                    .filter(|params| !params.is_empty());
                let params = non_empty_params.map(|params| {
                    params
                        .iter()
                        .map(|(name, value)| {
                            (name.clone(), crate::ConfigValue::from(toml_to_json(value)))
                        })
                        .collect()
                });

                crate::ConfigRule {
                    name: rule.name.to_owned(),
                    description: rule.description.to_owned(),
                    severity: rule.severity.as_str().to_owned(),
                    category: rule.kind.as_str().to_owned(),
                    fixable: rule.fixable,
                    enabled: configured.is_some_and(|config| config.enabled),
                    only: configured
                        .map(|config| config.only.clone())
                        .unwrap_or_default(),
                    overrides: configured
                        .map(|config| config.overrides.clone())
                        .unwrap_or_default(),
                    ignore: configured
                        .map(|config| config.ignore.clone())
                        .unwrap_or_default(),
                    params,
                }
            })
            .collect()
    }

    /// Return a configured integer parameter, or its declared default.
    #[must_use]
    pub fn get_i64(&self, rule: &str, param: &RuleParam, rel: &str) -> i64 {
        self.parameter(rule, param, rel)
            .and_then(toml::Value::as_integer)
            .unwrap_or_else(|| match param.default {
                ParamDefault::Int(d) => d,

                ParamDefault::StringArray(_) | ParamDefault::Bool(_) => {
                    unreachable!("get_i64 is only called for Int params")
                }
            })
    }

    /// Return a path-resolved boolean parameter, or its declared default.
    #[must_use]
    pub fn get_bool(&self, rule: &str, param: &RuleParam, rel: &str) -> bool {
        let default = matches!(param.default, ParamDefault::Bool(true));

        self.parameter(rule, param, rel)
            .and_then(toml::Value::as_bool)
            .unwrap_or(default)
    }

    // #rw(rust_getter_prefix) keyed param lookup mirrors the toml::Value getter family
    pub(crate) fn get_u64(&self, rule: &str, param: &RuleParam, rel: &str) -> u64 {
        u64::try_from(self.get_i64(rule, param, rel)).unwrap_or_default()
    }

    // #rw(rust_getter_prefix) keyed param lookup mirrors the toml::Value getter family
    pub fn get_usize(&self, rule: &str, param: &RuleParam, rel: &str) -> usize {
        usize::try_from(self.get_i64(rule, param, rel)).unwrap_or_default()
    }

    pub(crate) fn workspace(&self) -> &super::workspace::WorkspaceContext {
        &self.workspace
    }

    fn insert_default(&mut self, name: &str, params: &'static [RuleParam], enabled: bool) {
        if self.rules.contains_key(name) {
            return;
        }

        let params = params
            .iter()
            .map(|parameter| {
                let value = match &parameter.default {
                    ParamDefault::Int(default) => toml::Value::Integer(*default),

                    ParamDefault::Bool(default) => toml::Value::Boolean(*default),

                    ParamDefault::StringArray(default) => toml::Value::Array(
                        default
                            .iter()
                            .map(|value| toml::Value::String((*value).to_owned()))
                            .collect(),
                    ),
                };

                (parameter.name.to_owned(), value)
            })
            .collect();

        self.rules.insert(
            name.to_owned(),
            RuleConfig {
                enabled,
                ignore: Vec::new(),
                only: Vec::new(),
                overrides: Vec::new(),
                params,
            },
        );
    }

    fn resolve(raw: RawConfig) -> Result<Config, ConfigError> {
        let mut rules = BTreeMap::new();
        let mut errors = Vec::new();

        for (set_name, patterns) in &raw.glob_sets {
            for pattern in patterns {
                if let Err(error) = crate::glob::validate(pattern) {
                    errors.push(format!(
                        "glob set `{set_name}` contains invalid pattern `{pattern}`: {error}"
                    ));
                }
            }
        }

        for (rule_name, raw_rule) in &raw.rules {
            let mut ignore = Vec::new();

            for entry in &raw_rule.ignore {
                let explicit_set = entry.strip_prefix('@');
                let set_name = explicit_set.unwrap_or(entry);

                match raw.glob_sets.get(set_name) {
                    Some(patterns) => {
                        for p in patterns {
                            if !ignore.contains(p) {
                                ignore.push(p.clone());
                            }
                        }
                    }

                    None if explicit_set.is_some() => {
                        errors.push(format!(
                            "rule `{rule_name}` references unknown glob set `{set_name}` — \
                             add it to [glob_sets] or fix the typo"
                        ));
                    }

                    None if !ignore.contains(entry) => {
                        if let Err(error) = crate::glob::validate(entry) {
                            errors.push(format!(
                                "rule `{rule_name}` contains invalid ignore pattern `{entry}`: {error}"
                            ));
                        }

                        ignore.push(entry.clone());
                    }

                    None => {}
                }
            }

            rules.insert(
                rule_name.clone(),
                RuleConfig {
                    enabled: raw_rule.enabled,
                    ignore,
                    only: resolve_patterns(&raw_rule.only, &raw.glob_sets, rule_name, &mut errors),
                    overrides: raw_rule
                        .overrides
                        .iter()
                        .map(|entry| {
                            let paths = resolve_patterns(
                                &entry.paths,
                                &raw.glob_sets,
                                rule_name,
                                &mut errors,
                            );

                            if paths.is_empty() {
                                errors.push(format!(
                                    "rule `{rule_name}` override paths must not be empty"
                                ));
                            }

                            RuleOverride {
                                paths,
                                params: entry.params.clone(),
                            }
                        })
                        .collect(),
                    params: raw_rule.params.clone(),
                },
            );
        }

        if !errors.is_empty() {
            return Err(ConfigError::new(ConfigErrorKind::Resolve {
                message: errors.join("\n"),
            }));
        }

        Ok(Config {
            allow_suppressions: raw.allow_suppressions,
            glob_sets: raw.glob_sets,
            rules,
            workspace: super::workspace::WorkspaceContext::default(),
        })
    }

    #[cfg(test)]
    pub(crate) const fn with_suppressions_allowed(mut self, allowed: bool) -> Self {
        self.allow_suppressions = allowed;

        self
    }

    fn validate_param(rule: &str, param: &RuleParam, cfg: &RuleConfig, errors: &mut Vec<String>) {
        match cfg.params.get(param.name) {
            Some(val) => {
                let ok = match param.param_type {
                    ParamType::Bool => val.as_bool().is_some(),

                    ParamType::Int => val.as_integer().is_some_and(|value| value >= 0),

                    ParamType::StringArray => val.as_array().is_some_and(|values| {
                        let strings: Option<Vec<&str>> =
                            values.iter().map(toml::Value::as_str).collect();

                        strings.is_some_and(|strings| {
                            let allowed = param.allowed_values;
                            let values_allowed = allowed.is_empty()
                                || strings.iter().all(|value| allowed.contains(value));
                            let mut deduplicated = strings.clone();

                            deduplicated.sort_unstable();
                            deduplicated.dedup();

                            values_allowed && deduplicated.len() == strings.len()
                        })
                    }),
                };

                if !ok {
                    let expected = match param.param_type {
                        ParamType::Bool => "a boolean",

                        ParamType::Int => "a non-negative integer",

                        ParamType::StringArray if !param.allowed_values.is_empty() => {
                            "a duplicate-free array of allowed strings"
                        }

                        ParamType::StringArray => "an array of strings",
                    };

                    errors.push(format!(
                        "rule `{rule}` param `{}` must be {expected}",
                        param.name
                    ));
                } else if let Some(values) = val.as_array() {
                    let strings = values
                        .iter()
                        .filter_map(toml::Value::as_str)
                        .map(str::to_owned)
                        .collect::<Vec<_>>();

                    if rule == "toml_cargo_layers" && param.name == "rules" {
                        if let Err(error) =
                            crate::languages::toml::rules::cargo::layers::validate_edges(&strings)
                        {
                            errors.push(format!("rule `{rule}`: {error}"));
                        }
                    } else if rule == "toml_cargo_path_deps" && param.name == "allow_in" {
                        for pattern in strings {
                            if let Err(error) = crate::glob::validate(&pattern) {
                                errors.push(format!("rule `{rule}`: {error}"));
                            }
                        }
                    }
                }
            }

            None => {
                errors.push(format!(
                    "rule `{rule}` is missing required param `{}` — \
                     add it to rulewright.toml or run `rulewright --init` to regenerate",
                    param.name
                ));
            }
        }
    }
}

fn toml_to_json(value: &toml::Value) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
}

fn resolve_patterns(
    entries: &[String],
    sets: &BTreeMap<String, Vec<String>>,
    rule: &str,
    errors: &mut Vec<String>,
) -> Vec<String> {
    let mut result = Vec::new();

    for entry in entries {
        if let Some(patterns) = sets.get(entry.strip_prefix('@').unwrap_or(entry)) {
            result.extend(patterns.iter().cloned());
        } else if entry.starts_with('@') {
            errors.push(format!(
                "rule `{rule}` references unknown glob set `{entry}`"
            ));
        } else {
            if let Err(error) = crate::glob::validate(entry) {
                errors.push(format!(
                    "rule `{rule}` contains invalid pattern `{entry}`: {error}"
                ));
            }

            result.push(entry.clone());
        }
    }

    result.sort();
    result.dedup();
    result
}

#[cfg(test)]
mod tests;
