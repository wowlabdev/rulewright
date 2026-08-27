#[macro_export]
#[doc(hidden)]
macro_rules! _declare_params {
    () => {
        const PARAMS: &[$crate::RuleParam] = &[];
    };
    ($pname:ident : [String] = $default:tt in $allowed:tt $(,)?) => {
        const PARAMS: &[$crate::RuleParam] = &[
            $crate::_param!($pname : [String] = $default in $allowed),
        ];
    };
    ($($pname:ident : $ptype:tt = $default:tt),+ $(,)?) => {
        const PARAMS: &[$crate::RuleParam] = &[
            $($crate::_param!($pname : $ptype = $default)),+
        ];
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _param {
    ($pname:ident : i64 = $default:expr) => {
        $crate::RuleParam {
            name: stringify!($pname),
            param_type: $crate::ParamType::Int,
            default: $crate::ParamDefault::Int($default),
            allowed_values: &[],
        }
    };
    ($pname:ident : [String] = [$($val:expr),* $(,)?] in [$($allowed:expr),* $(,)?]) => {
        $crate::RuleParam {
            name: stringify!($pname),
            param_type: $crate::ParamType::StringArray,
            default: $crate::ParamDefault::StringArray(&[$($val),*]),
            allowed_values: &[$($allowed),*],
        }
    };
    ($pname:ident : [String] = [$($val:expr),* $(,)?]) => {
        $crate::RuleParam {
            name: stringify!($pname),
            param_type: $crate::ParamType::StringArray,
            default: $crate::ParamDefault::StringArray(&[$($val),*]),
            allowed_values: &[],
        }
    };
}

/// Run every `&[Example]` as one test, asserting each pass/fail expectation.
#[macro_export]
#[doc(hidden)]
macro_rules! example_tests {
    ($examples:expr, $check_fn:ident) => {
        #[$crate::_private::gtest]
        fn examples() -> $crate::_private::TestResult {
            for (i, ex) in $examples.iter().enumerate() {
                $crate::_private::scoped_trace!("example[{i}] {:?}:\n{}", ex.label, ex.code);
                let violations = run(ex.code);
                if ex.pass {
                    $crate::_private::verify_true!(violations.is_empty())?;
                } else {
                    $crate::_private::verify_false!(violations.is_empty())?;
                }
            }

            Ok(())
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! rulewright_test {
    ($check_fn:ident, { $($body:tt)* }) => {
        #[cfg(test)]
        mod tests {
            use super::*;
            fn run(source: &str) -> Vec<$crate::Violation> {
                $crate::check_source(source, $check_fn)
            }
            $($body)*
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! rulewright_ast_test {
    ($check_fn:ident, { $($body:tt)* }) => {
        #[cfg(test)]
        mod tests {
            use super::*;
            fn run(source: &str) -> Vec<$crate::Violation> {
                $crate::check_source_ast(source, $check_fn)
            }
            $($body)*
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! rulewright_toml_test {
    ($check_fn:ident, { $($body:tt)* }) => {
        #[cfg(test)]
        mod tests {
            use super::*;
            fn run(source: &str) -> Vec<$crate::Violation> {
                $crate::check_source_toml(source, $check_fn)
            }
            $($body)*
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! rulewright_workspace_test {
    ($check_fn:ident, { $($body:tt)* }) => {
        #[cfg(test)]
        mod tests {
            use super::*;
            fn run(source: &str) -> Vec<$crate::Violation> {
                $crate::check_workspace_sources(&[("test.rs", source)], $check_fn)
            }
            $($body)*
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _rule_info {
    ($name:expr, $desc:expr, $just:expr, $sev:ident, $params:expr) => {
        $crate::RuleInfo {
            name: $name,
            description: $desc,
            justification: $just,
            severity: $crate::Severity::$sev,
            examples: EXAMPLES,
            params: $params,
            default_enabled: true,
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _rule_info_disabled {
    ($name:expr, $desc:expr, $just:expr, $sev:ident, $params:expr) => {
        $crate::RuleInfo {
            name: $name,
            description: $desc,
            justification: $just,
            severity: $crate::Severity::$sev,
            examples: EXAMPLES,
            params: $params,
            default_enabled: false,
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _submit_ast_rule_disabled {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr) => {
        paste::paste! {
            inventory::submit! {
                $crate::Rule {
                    info: $crate::_rule_info_disabled!(concat!("rust_", stringify!($name)), $desc, $just, $sev, $params),
                    check: $crate::RuleCheck::RustAst([<check_ $name>]),
                    fix: None,
                }
            }
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _submit_line_rule_disabled {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr) => {
        paste::paste! {
            inventory::submit! {
                $crate::Rule {
                    info: $crate::_rule_info_disabled!(concat!("rust_", stringify!($name)), $desc, $just, $sev, $params),
                    check: $crate::RuleCheck::RustLine([<check_ $name>]),
                    fix: None,
                }
            }
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _submit_line_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr, $fix:expr) => {
        paste::paste! {
            inventory::submit! {
                $crate::Rule {
                    info: $crate::_rule_info!(concat!("rust_", stringify!($name)), $desc, $just, $sev, $params),
                    check: $crate::RuleCheck::RustLine([<check_ $name>]),
                    fix: $fix,
                }
            }
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _submit_full_line_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr, $fix:expr) => {
        paste::paste! {
            inventory::submit! {
                $crate::Rule {
                    info: $crate::_rule_info!(concat!("rust_", stringify!($name)), $desc, $just, $sev, $params),
                    check: $crate::RuleCheck::RustLineFull([<check_ $name>]),
                    fix: $fix,
                }
            }
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _submit_ast_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr, $fix:expr) => {
        paste::paste! {
            inventory::submit! {
                $crate::Rule {
                    info: $crate::_rule_info!(concat!("rust_", stringify!($name)), $desc, $just, $sev, $params),
                    check: $crate::RuleCheck::RustAst([<check_ $name>]),
                    fix: $fix,
                }
            }
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _submit_ast_tree_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr, $fix:expr) => {
        paste::paste! {
            inventory::submit! {
                $crate::Rule {
                    info: $crate::_rule_info!(concat!("rust_", stringify!($name)), $desc, $just, $sev, $params),
                    check: $crate::RuleCheck::RustAst([<check_ $name>]),
                    fix: Some($crate::RuleFix::RustAstTree($fix)),
                }
            }
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _submit_workspace_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr) => {
        paste::paste! {
            inventory::submit! {
                $crate::Rule {
                    info: $crate::_rule_info!(concat!("rust_", stringify!($name)), $desc, $just, $sev, $params),
                    check: $crate::RuleCheck::RustWorkspace([<check_ $name>]),
                    fix: None,
                }
            }
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _submit_language_workspace_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr) => {
        paste::paste! {
            inventory::submit! {
                $crate::Rule {
                    info: $crate::_rule_info!(stringify!($name), $desc, $just, $sev, $params),
                    check: $crate::RuleCheck::Workspace([<check_ $name>]),
                    fix: None,
                }
            }
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _submit_toml_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr, $fix:expr) => {
        paste::paste! {
            inventory::submit! {
                $crate::Rule {
                    info: $crate::_rule_info!(stringify!($name), $desc, $just, $sev, $params),
                    check: $crate::RuleCheck::Toml([<check_ $name>]),
                    fix: $fix,
                }
            }
        }
    };
}

/// Register a line-based lint rule.
#[macro_export]
#[doc(hidden)]
macro_rules! line_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, default = false $(,)?) => {
        $crate::_submit_line_rule_disabled!($name, $desc, $just, $sev, &[]);
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($($params)*);
        $crate::_submit_line_rule!($name, $desc, $just, $sev, PARAMS, Some($crate::RuleFix::RustLine($fix)));
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($($params)*);
        $crate::_submit_line_rule!($name, $desc, $just, $sev, PARAMS, None);
    };
    ($name:ident, $desc:expr, $just:expr, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($($params)*);
        $crate::_submit_line_rule!($name, $desc, $just, Low, PARAMS, None);
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr $(,)?) => {
        $crate::_submit_line_rule!($name, $desc, $just, $sev, &[], Some($crate::RuleFix::RustLine($fix)));
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident $(,)?) => {
        $crate::_submit_line_rule!($name, $desc, $just, $sev, &[], None);
    };
    ($name:ident, $desc:expr, $just:expr $(,)?) => {
        $crate::_submit_line_rule!($name, $desc, $just, Low, &[], None);
    };
}

/// Register a line rule that intentionally sees test-only source as well as production source.
#[macro_export]
#[doc(hidden)]
macro_rules! full_line_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr $(,)?) => {
        $crate::_submit_full_line_rule!(
            $name,
            $desc,
            $just,
            $sev,
            &[],
            Some($crate::RuleFix::RustLine($fix))
        );
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident $(,)?) => {
        $crate::_submit_full_line_rule!($name, $desc, $just, $sev, &[], None);
    };
}

/// Register an AST-based lint rule.
#[macro_export]
#[doc(hidden)]
macro_rules! ast_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($($params)*);
        $crate::_submit_ast_rule!($name, $desc, $just, $sev, PARAMS, None);
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, default = false, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($($params)*);
        $crate::_submit_ast_rule_disabled!($name, $desc, $just, $sev, PARAMS);
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, default = false $(,)?) => {
        $crate::_submit_ast_rule_disabled!($name, $desc, $just, $sev, &[]);
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($($params)*);
        $crate::_submit_ast_rule!($name, $desc, $just, $sev, PARAMS, Some($crate::RuleFix::RustAst($fix)));
    };
    ($name:ident, $desc:expr, $just:expr, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($($params)*);
        $crate::_submit_ast_rule!($name, $desc, $just, Low, PARAMS, None);
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr $(,)?) => {
        $crate::_submit_ast_rule!($name, $desc, $just, $sev, &[], Some($crate::RuleFix::RustAst($fix)));
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident $(,)?) => {
        $crate::_submit_ast_rule!($name, $desc, $just, $sev, &[], None);
    };
    ($name:ident, $desc:expr, $just:expr $(,)?) => {
        $crate::_submit_ast_rule!($name, $desc, $just, Low, &[], None);
    };
}

/// Register a rust-analyzer AST rule whose fixer produces one coordinated tree edit per file.
#[macro_export]
#[doc(hidden)]
macro_rules! ast_tree_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($($params)*);
        $crate::_submit_ast_tree_rule!($name, $desc, $just, $sev, PARAMS, $fix);
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr $(,)?) => {
        $crate::_submit_ast_tree_rule!($name, $desc, $just, $sev, &[], $fix);
    };
}

/// Register a cross-file Rust rule.
#[macro_export]
#[doc(hidden)]
macro_rules! workspace_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($($params)*);
        $crate::_submit_workspace_rule!($name, $desc, $just, $sev, PARAMS);
    };
}

/// Register a language-neutral cross-file workspace rule.
#[macro_export]
#[doc(hidden)]
macro_rules! language_workspace_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($($params)*);
        $crate::_submit_language_workspace_rule!($name, $desc, $just, $sev, PARAMS);
    };
}

/// Register a Taplo-backed TOML rule.
#[macro_export]
#[doc(hidden)]
macro_rules! toml_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($($params)*);
        $crate::_submit_toml_rule!($name, $desc, $just, $sev, PARAMS, Some($crate::RuleFix::Toml($fix)));
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($($params)*);
        $crate::_submit_toml_rule!($name, $desc, $just, $sev, PARAMS, None);
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr $(,)?) => {
        $crate::_submit_toml_rule!($name, $desc, $just, $sev, &[], Some($crate::RuleFix::Toml($fix)));
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident $(,)?) => {
        $crate::_submit_toml_rule!($name, $desc, $just, $sev, &[], None);
    };
}

/// Verify fix functions resolve all violations in failing examples.
#[macro_export]
#[doc(hidden)]
macro_rules! fix_tests {
    (line, $check_fn:ident, $fix_fn:ident) => {
        #[$crate::_private::gtest]
        fn fixes() -> $crate::_private::TestResult {
            for (i, ex) in EXAMPLES.iter().enumerate() {
                if ex.pass {
                    continue;
                }
                let fixed = $crate::apply_line_fixes(ex.code, $check_fn, $fix_fn);
                let remaining = run(&fixed);
                $crate::_private::scoped_trace!("example {i} ({:?})", ex.label);
                $crate::_private::verify_true!(remaining.is_empty())?;
            }

            Ok(())
        }
    };
    (ast, $check_fn:ident, $fix_fn:ident) => {
        #[$crate::_private::gtest]
        fn fixes() -> $crate::_private::TestResult {
            for (i, ex) in EXAMPLES.iter().enumerate() {
                if ex.pass {
                    continue;
                }
                let fixed = $crate::apply_ast_fixes(ex.code, $check_fn, $fix_fn);
                let remaining = run(&fixed);
                $crate::_private::scoped_trace!("example {i} ({:?})", ex.label);
                $crate::_private::verify_true!(remaining.is_empty())?;
            }

            Ok(())
        }
    };
    (ast_tree, $check_fn:ident, $fix_fn:ident) => {
        #[$crate::_private::gtest]
        fn fixes() -> $crate::_private::TestResult {
            for (i, ex) in EXAMPLES.iter().enumerate() {
                if ex.pass {
                    continue;
                }
                let fixed = $crate::apply_ast_tree_fix(ex.code, $check_fn, $fix_fn);
                let remaining = run(&fixed);
                $crate::_private::scoped_trace!("example {i} ({:?})", ex.label);
                $crate::_private::verify_true!(remaining.is_empty())?;
            }

            Ok(())
        }
    };
}
