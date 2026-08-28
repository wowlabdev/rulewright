#[macro_export]
#[doc(hidden)]
macro_rules! _declare_params {
    ($rule:ident; $($params:tt)*) => {
        $crate::_declare_params!(@collect $rule; []; $($params)*,);
    };
    (@collect $rule:ident; [$($declared:tt)*];) => {
        $crate::__paste::paste! {
            const [<$rule:upper _PARAMS>]: &[$crate::RuleParam] = &[
                $($declared)*
            ];
        }
    };
    (@collect $rule:ident; [$($declared:tt)*]; , $($rest:tt)*) => {
        $crate::_declare_params!(@collect $rule; [$($declared)*]; $($rest)*);
    };
    (@collect $rule:ident; [$($declared:tt)*]; $pname:ident : [String] = $default:tt in $allowed:tt, $($rest:tt)*) => {
        $crate::_declare_params!(@collect $rule; [
            $($declared)*
            $crate::_param!($pname : [String] = $default in $allowed),
        ]; $($rest)*);
    };
    (@collect $rule:ident; [$($declared:tt)*]; $pname:ident : i64 = - $default:literal, $($rest:tt)*) => {
        $crate::_declare_params!(@collect $rule; [
            $($declared)*
            $crate::_param!($pname : i64 = -$default),
        ]; $($rest)*);
    };
    (@collect $rule:ident; [$($declared:tt)*]; $pname:ident : i64 = $default:tt in $allowed:tt, $($rest:tt)*) => {
        compile_error!(concat!(
            "integer parameter `",
            stringify!($pname),
            "` cannot declare `in ",
            stringify!($allowed),
            "`; allowed-value lists are supported only for `[String]` parameters"
        ));
    };
    (@collect $rule:ident; [$($declared:tt)*]; $pname:ident : $ptype:tt = $default:tt, $($rest:tt)*) => {
        $crate::_declare_params!(@collect $rule; [
            $($declared)*
            $crate::_param!($pname : $ptype = $default),
        ]; $($rest)*);
    };
    (@collect $rule:ident; [$($declared:tt)*]; $($unsupported:tt)+) => {
        compile_error!(concat!(
            "unsupported Rulewright parameter declaration `",
            stringify!($($unsupported)*),
            "`; supported types are `i64` and `[String]`"
        ));
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
    ($pname:ident : $($unsupported:tt)*) => {
        compile_error!(concat!(
            "unsupported Rulewright parameter type in `",
            stringify!($pname : $($unsupported)*),
            "`; supported types are `i64` and `[String]`"
        ))
    };
}

/// Run every `&[Example]` as one test, asserting each pass/fail expectation.
///
/// Use this inside a `rulewright_*_test!` body, which provides the fixture's `run` function.
#[macro_export]
#[doc(hidden)]
macro_rules! example_tests {
    ($examples:expr, $check_fn:ident) => {
        $crate::__paste::paste! {
            #[cfg(test)]
            #[$crate::_private::gtest]
            fn [<examples_ $check_fn>]() -> $crate::_private::TestResult {
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
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! rulewright_test {
    ($check_fn:ident, { $($body:tt)* }) => {
        $crate::__paste::paste! {
            #[cfg(test)]
            mod [<tests_ $check_fn>] {
                use super::*;
                fn run(source: &str) -> Vec<$crate::Violation> {
                    $crate::testing::check_source(source, $check_fn)
                }
                $($body)*
            }
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! rulewright_ast_test {
    ($check_fn:ident, { $($body:tt)* }) => {
        $crate::__paste::paste! {
            #[cfg(test)]
            mod [<tests_ $check_fn>] {
                use super::*;
                fn run(source: &str) -> Vec<$crate::Violation> {
                    $crate::testing::check_source_ast(source, $check_fn)
                }
                $($body)*
            }
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! rulewright_toml_test {
    ($check_fn:ident, { $($body:tt)* }) => {
        $crate::__paste::paste! {
            #[cfg(test)]
            mod [<tests_ $check_fn>] {
                use super::*;
                fn run(source: &str) -> Vec<$crate::Violation> {
                    $crate::testing::check_source_toml(source, $check_fn)
                }
                $($body)*
            }
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! rulewright_toml_test_at {
    ($rel:expr, $check_fn:ident, { $($body:tt)* }) => {
        $crate::__paste::paste! {
            #[cfg(test)]
            mod [<tests_ $check_fn>] {
                use super::*;
                fn run(source: &str) -> Vec<$crate::Violation> {
                    $crate::testing::check_source_toml_at($rel, source, $check_fn)
                }
                $($body)*
            }
        }
    };
}

/// Declare a Rust line rule owned by a statically linked downstream rule pack.
///
/// The surrounding module must define an `EXAMPLES: &[rulewright::Example]` constant.
/// The macro emits `<RULE_NAME>_RULE` and, when configured, `<RULE_NAME>_PARAMS` constants.
#[macro_export]
macro_rules! pack_line_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, default = false, params { $($params:tt)* } $(,)?) => {
        $crate::_line_rule!(pack_disabled, $name, $desc, $just, $sev, None, params { $($params)* });
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, default = false $(,)?) => {
        $crate::_line_rule!(pack_disabled, $name, $desc, $just, $sev, None);
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, params { $($params:tt)* } $(,)?) => {
        $crate::_line_rule!(pack, $name, $desc, $just, $sev, None, params { $($params)* });
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident $(,)?) => {
        $crate::_line_rule!(pack, $name, $desc, $just, $sev, None);
    };
}

/// Declare a Rust AST rule owned by a statically linked downstream rule pack.
///
/// The surrounding module must define an `EXAMPLES: &[rulewright::Example]` constant.
/// The macro emits `<RULE_NAME>_RULE` and, when configured, `<RULE_NAME>_PARAMS` constants.
#[macro_export]
macro_rules! pack_ast_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, default = false, params { $($params:tt)* } $(,)?) => {
        $crate::_ast_rule!(pack_disabled, $name, $desc, $just, $sev, None, params { $($params)* });
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, default = false $(,)?) => {
        $crate::_ast_rule!(pack_disabled, $name, $desc, $just, $sev, None);
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, params { $($params:tt)* } $(,)?) => {
        $crate::_ast_rule!(pack, $name, $desc, $just, $sev, None, params { $($params)* });
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident $(,)?) => {
        $crate::_ast_rule!(pack, $name, $desc, $just, $sev, None);
    };
}

/// Declare a TOML rule owned by a statically linked downstream rule pack.
///
/// The surrounding module must define an `EXAMPLES: &[rulewright::Example]` constant.
/// The macro emits `<RULE_NAME>_RULE` and, when configured, `<RULE_NAME>_PARAMS` constants.
#[macro_export]
macro_rules! pack_toml_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, default = false, params { $($params:tt)* } $(,)?) => {
        $crate::_toml_rule!(pack_disabled, $name, $desc, $just, $sev, None, params { $($params)* });
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, default = false $(,)?) => {
        $crate::_toml_rule!(pack_disabled, $name, $desc, $just, $sev, None);
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr, params { $($params:tt)* } $(,)?) => {
        $crate::_toml_rule!(pack, $name, $desc, $just, $sev, Some($crate::RuleFix::Toml($fix)), params { $($params)* });
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, params { $($params:tt)* } $(,)?) => {
        $crate::_toml_rule!(pack, $name, $desc, $just, $sev, None, params { $($params)* });
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr $(,)?) => {
        $crate::_toml_rule!(pack, $name, $desc, $just, $sev, Some($crate::RuleFix::Toml($fix)));
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident $(,)?) => {
        $crate::_toml_rule!(pack, $name, $desc, $just, $sev, None);
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! rulewright_workspace_test {
    ($check_fn:ident, { $($body:tt)* }) => {
        $crate::__paste::paste! {
            #[cfg(test)]
            mod [<tests_ $check_fn>] {
                use super::*;
                fn run(source: &str) -> Vec<$crate::Violation> {
                    $crate::check_workspace_sources(&[("test.rs", source)], $check_fn)
                }
                $($body)*
            }
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
macro_rules! _define_rule {
    (inventory, $info:expr, $kind:ident, $check:expr, $fix:expr) => {
        $crate::__inventory::submit! {
            $crate::Rule {
                info: $info,
                check: $crate::RuleCheck::$kind($check),
                fix: $fix,
            }
        }
    };
    (pack, $name:ident, $info:expr, $kind:ident, $check:expr, $fix:expr) => {
        $crate::__paste::paste! {
            pub(crate) static [<$name:upper _RULE>]: $crate::Rule = $crate::Rule {
                info: $info,
                check: $crate::RuleCheck::$kind($check),
                fix: $fix,
            };
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _submit_ast_rule_disabled {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr) => {
        $crate::__paste::paste! {
            $crate::_define_rule!(inventory, $crate::_rule_info_disabled!(concat!("rust_", stringify!($name)), $desc, $just, $sev, $params), RustAst, [<check_ $name>], None);
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _submit_line_rule_disabled {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr) => {
        $crate::__paste::paste! {
            $crate::_define_rule!(inventory, $crate::_rule_info_disabled!(concat!("rust_", stringify!($name)), $desc, $just, $sev, $params), RustLine, [<check_ $name>], None);
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _submit_line_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr, $fix:expr) => {
        $crate::__paste::paste! {
            $crate::_define_rule!(inventory, $crate::_rule_info!(concat!("rust_", stringify!($name)), $desc, $just, $sev, $params), RustLine, [<check_ $name>], $fix);
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _submit_full_line_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr, $fix:expr) => {
        $crate::__paste::paste! {
            $crate::__inventory::submit! {
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
macro_rules! _submit_full_line_rule_disabled {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr) => {
        $crate::__paste::paste! {
            $crate::_define_rule!(inventory, $crate::_rule_info_disabled!(concat!("rust_", stringify!($name)), $desc, $just, $sev, $params), RustLineFull, [<check_ $name>], None);
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _submit_ast_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr, $fix:expr) => {
        $crate::__paste::paste! {
            $crate::_define_rule!(inventory, $crate::_rule_info!(concat!("rust_", stringify!($name)), $desc, $just, $sev, $params), RustAst, [<check_ $name>], $fix);
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _submit_ast_tree_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr, $fix:expr) => {
        $crate::__paste::paste! {
            $crate::__inventory::submit! {
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
        $crate::__paste::paste! {
            $crate::__inventory::submit! {
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
        $crate::__paste::paste! {
            $crate::__inventory::submit! {
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
        $crate::__paste::paste! {
            $crate::_define_rule!(inventory, $crate::_rule_info!(stringify!($name), $desc, $just, $sev, $params), Toml, [<check_ $name>], $fix);
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _submit_toml_rule_disabled {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr) => {
        $crate::__paste::paste! {
            $crate::_define_rule!(inventory, $crate::_rule_info_disabled!(stringify!($name), $desc, $just, $sev, $params), Toml, [<check_ $name>], None);
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _declare_pack_line_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr) => {
        $crate::__paste::paste! {
            $crate::_define_rule!(pack, $name, $crate::_rule_info!(concat!("rust_", stringify!($name)), $desc, $just, $sev, $params), RustLine, [<check_ $name>], None);
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _declare_pack_ast_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr) => {
        $crate::__paste::paste! {
            $crate::_define_rule!(pack, $name, $crate::_rule_info!(concat!("rust_", stringify!($name)), $desc, $just, $sev, $params), RustAst, [<check_ $name>], None);
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _declare_pack_line_rule_disabled {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr) => {
        $crate::__paste::paste! {
            $crate::_define_rule!(pack, $name, $crate::_rule_info_disabled!(concat!("rust_", stringify!($name)), $desc, $just, $sev, $params), RustLine, [<check_ $name>], None);
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _declare_pack_ast_rule_disabled {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr) => {
        $crate::__paste::paste! {
            $crate::_define_rule!(pack, $name, $crate::_rule_info_disabled!(concat!("rust_", stringify!($name)), $desc, $just, $sev, $params), RustAst, [<check_ $name>], None);
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _declare_pack_toml_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr, $fix:expr) => {
        $crate::__paste::paste! {
            $crate::_define_rule!(pack, $name, $crate::_rule_info!(stringify!($name), $desc, $just, $sev, $params), Toml, [<check_ $name>], $fix);
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _declare_pack_toml_rule_disabled {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr) => {
        $crate::__paste::paste! {
            $crate::_define_rule!(pack, $name, $crate::_rule_info_disabled!(stringify!($name), $desc, $just, $sev, $params), Toml, [<check_ $name>], None);
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _line_rule {
    ($target:ident, $name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($name; $($params)*);
        $crate::__paste::paste! {
            $crate::_line_rule!(@emit $target, $name, $desc, $just, $sev, [<$name:upper _PARAMS>], $fix);
        }
    };
    ($target:ident, $name:ident, $desc:expr, $just:expr, $sev:ident, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($name; $($params)*);
        $crate::__paste::paste! {
            $crate::_line_rule!(@emit $target, $name, $desc, $just, $sev, [<$name:upper _PARAMS>], None);
        }
    };
    ($target:ident, $name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr $(,)?) => {
        $crate::_line_rule!(@emit $target, $name, $desc, $just, $sev, &[], $fix);
    };
    (@emit inventory, $name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr, $fix:expr) => {
        $crate::_submit_line_rule!($name, $desc, $just, $sev, $params, $fix);
    };
    (@emit pack, $name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr, $fix:expr) => {
        $crate::_declare_pack_line_rule!($name, $desc, $just, $sev, $params);
    };
    (@emit pack_disabled, $name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr, $fix:expr) => {
        $crate::_declare_pack_line_rule_disabled!($name, $desc, $just, $sev, $params);
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _ast_rule {
    ($target:ident, $name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($name; $($params)*);
        $crate::__paste::paste! {
            $crate::_ast_rule!(@emit $target, $name, $desc, $just, $sev, [<$name:upper _PARAMS>], $fix);
        }
    };
    ($target:ident, $name:ident, $desc:expr, $just:expr, $sev:ident, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($name; $($params)*);
        $crate::__paste::paste! {
            $crate::_ast_rule!(@emit $target, $name, $desc, $just, $sev, [<$name:upper _PARAMS>], None);
        }
    };
    ($target:ident, $name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr $(,)?) => {
        $crate::_ast_rule!(@emit $target, $name, $desc, $just, $sev, &[], $fix);
    };
    (@emit inventory, $name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr, $fix:expr) => {
        $crate::_submit_ast_rule!($name, $desc, $just, $sev, $params, $fix);
    };
    (@emit pack, $name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr, $fix:expr) => {
        $crate::_declare_pack_ast_rule!($name, $desc, $just, $sev, $params);
    };
    (@emit pack_disabled, $name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr, $fix:expr) => {
        $crate::_declare_pack_ast_rule_disabled!($name, $desc, $just, $sev, $params);
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! _toml_rule {
    ($target:ident, $name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($name; $($params)*);
        $crate::__paste::paste! {
            $crate::_toml_rule!(@emit $target, $name, $desc, $just, $sev, [<$name:upper _PARAMS>], $fix);
        }
    };
    ($target:ident, $name:ident, $desc:expr, $just:expr, $sev:ident, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($name; $($params)*);
        $crate::__paste::paste! {
            $crate::_toml_rule!(@emit $target, $name, $desc, $just, $sev, [<$name:upper _PARAMS>], None);
        }
    };
    ($target:ident, $name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr $(,)?) => {
        $crate::_toml_rule!(@emit $target, $name, $desc, $just, $sev, &[], $fix);
    };
    (@emit inventory, $name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr, $fix:expr) => {
        $crate::_submit_toml_rule!($name, $desc, $just, $sev, $params, $fix);
    };
    (@emit pack, $name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr, $fix:expr) => {
        $crate::_declare_pack_toml_rule!($name, $desc, $just, $sev, $params, $fix);
    };
    (@emit inventory_disabled, $name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr, $fix:expr) => {
        $crate::_submit_toml_rule_disabled!($name, $desc, $just, $sev, $params);
    };
    (@emit pack_disabled, $name:ident, $desc:expr, $just:expr, $sev:ident, $params:expr, $fix:expr) => {
        $crate::_declare_pack_toml_rule_disabled!($name, $desc, $just, $sev, $params);
    };
}

/// Register a line-based lint rule.
///
/// The surrounding module must define an `EXAMPLES: &[crate::Example]` constant.
/// Parameterized rules receive a `<RULE_NAME>_PARAMS` constant.
#[macro_export]
#[doc(hidden)]
macro_rules! line_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, default = false, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($name; $($params)*);
        $crate::__paste::paste! {
            $crate::_submit_line_rule_disabled!($name, $desc, $just, $sev, [<$name:upper _PARAMS>]);
        }
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, default = false $(,)?) => {
        $crate::_submit_line_rule_disabled!($name, $desc, $just, $sev, &[]);
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr, params { $($params:tt)* } $(,)?) => {
        $crate::_line_rule!(inventory, $name, $desc, $just, $sev, Some($crate::RuleFix::RustLine($fix)), params { $($params)* });
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, params { $($params:tt)* } $(,)?) => {
        $crate::_line_rule!(inventory, $name, $desc, $just, $sev, params { $($params)* });
    };
    ($name:ident, $desc:expr, $just:expr, params { $($params:tt)* } $(,)?) => {
        $crate::_line_rule!(inventory, $name, $desc, $just, Low, params { $($params)* });
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr $(,)?) => {
        $crate::_line_rule!(inventory, $name, $desc, $just, $sev, Some($crate::RuleFix::RustLine($fix)));
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident $(,)?) => {
        $crate::_line_rule!(inventory, $name, $desc, $just, $sev, None);
    };
    ($name:ident, $desc:expr, $just:expr $(,)?) => {
        $crate::_line_rule!(inventory, $name, $desc, $just, Low, None);
    };
}

/// Register a line rule that intentionally sees test-only source as well as production source.
#[macro_export]
#[doc(hidden)]
macro_rules! full_line_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, default = false, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($name; $($params)*);
        $crate::__paste::paste! {
            $crate::_submit_full_line_rule_disabled!($name, $desc, $just, $sev, [<$name:upper _PARAMS>]);
        }
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, default = false $(,)?) => {
        $crate::_submit_full_line_rule_disabled!($name, $desc, $just, $sev, &[]);
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($name; $($params)*);
        $crate::__paste::paste! {
            $crate::_submit_full_line_rule!($name, $desc, $just, $sev, [<$name:upper _PARAMS>], Some($crate::RuleFix::RustLine($fix)));
        }
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($name; $($params)*);
        $crate::__paste::paste! {
            $crate::_submit_full_line_rule!($name, $desc, $just, $sev, [<$name:upper _PARAMS>], None);
        }
    };
    ($name:ident, $desc:expr, $just:expr, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($name; $($params)*);
        $crate::__paste::paste! {
            $crate::_submit_full_line_rule!($name, $desc, $just, Low, [<$name:upper _PARAMS>], None);
        }
    };
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
    ($name:ident, $desc:expr, $just:expr $(,)?) => {
        $crate::_submit_full_line_rule!($name, $desc, $just, Low, &[], None);
    };
}

/// Register an AST-based lint rule.
///
/// The surrounding module must define an `EXAMPLES: &[crate::Example]` constant.
/// Parameterized rules receive a `<RULE_NAME>_PARAMS` constant.
#[macro_export]
#[doc(hidden)]
macro_rules! ast_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, params { $($params:tt)* } $(,)?) => {
        $crate::_ast_rule!(inventory, $name, $desc, $just, $sev, params { $($params)* });
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, default = false, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($name; $($params)*);
        $crate::__paste::paste! {
            $crate::_submit_ast_rule_disabled!($name, $desc, $just, $sev, [<$name:upper _PARAMS>]);
        }
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, default = false $(,)?) => {
        $crate::_submit_ast_rule_disabled!($name, $desc, $just, $sev, &[]);
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr, params { $($params:tt)* } $(,)?) => {
        $crate::_ast_rule!(inventory, $name, $desc, $just, $sev, Some($crate::RuleFix::RustAst($fix)), params { $($params)* });
    };
    ($name:ident, $desc:expr, $just:expr, params { $($params:tt)* } $(,)?) => {
        $crate::_ast_rule!(inventory, $name, $desc, $just, Low, params { $($params)* });
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr $(,)?) => {
        $crate::_ast_rule!(inventory, $name, $desc, $just, $sev, Some($crate::RuleFix::RustAst($fix)));
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident $(,)?) => {
        $crate::_ast_rule!(inventory, $name, $desc, $just, $sev, None);
    };
    ($name:ident, $desc:expr, $just:expr $(,)?) => {
        $crate::_ast_rule!(inventory, $name, $desc, $just, Low, None);
    };
}

/// Register a rust-analyzer AST rule whose fixer produces one coordinated tree edit per file.
#[macro_export]
#[doc(hidden)]
macro_rules! ast_tree_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($name; $($params)*);
        $crate::__paste::paste! {
            $crate::_submit_ast_tree_rule!($name, $desc, $just, $sev, [<$name:upper _PARAMS>], $fix);
        }
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
        $crate::_declare_params!($name; $($params)*);
        $crate::__paste::paste! {
            $crate::_submit_workspace_rule!($name, $desc, $just, $sev, [<$name:upper _PARAMS>]);
        }
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident $(,)?) => {
        $crate::_submit_workspace_rule!($name, $desc, $just, $sev, &[]);
    };
}

/// Register a language-neutral cross-file workspace rule.
#[macro_export]
#[doc(hidden)]
macro_rules! language_workspace_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, params { $($params:tt)* } $(,)?) => {
        $crate::_declare_params!($name; $($params)*);
        $crate::__paste::paste! {
            $crate::_submit_language_workspace_rule!($name, $desc, $just, $sev, [<$name:upper _PARAMS>]);
        }
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident $(,)?) => {
        $crate::_submit_language_workspace_rule!($name, $desc, $just, $sev, &[]);
    };
}

/// Register a Taplo-backed TOML rule.
///
/// The surrounding module must define an `EXAMPLES: &[crate::Example]` constant.
/// Parameterized rules receive a `<RULE_NAME>_PARAMS` constant.
#[macro_export]
#[doc(hidden)]
macro_rules! toml_rule {
    ($name:ident, $desc:expr, $just:expr, $sev:ident, default = false, params { $($params:tt)* } $(,)?) => {
        $crate::_toml_rule!(inventory_disabled, $name, $desc, $just, $sev, None, params { $($params)* });
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, default = false $(,)?) => {
        $crate::_toml_rule!(inventory_disabled, $name, $desc, $just, $sev, None);
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr, params { $($params:tt)* } $(,)?) => {
        $crate::_toml_rule!(inventory, $name, $desc, $just, $sev, Some($crate::RuleFix::Toml($fix)), params { $($params)* });
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, params { $($params:tt)* } $(,)?) => {
        $crate::_toml_rule!(inventory, $name, $desc, $just, $sev, params { $($params)* });
    };
    ($name:ident, $desc:expr, $just:expr, params { $($params:tt)* } $(,)?) => {
        $crate::_toml_rule!(inventory, $name, $desc, $just, Low, params { $($params)* });
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident, $fix:expr $(,)?) => {
        $crate::_toml_rule!(inventory, $name, $desc, $just, $sev, Some($crate::RuleFix::Toml($fix)));
    };
    ($name:ident, $desc:expr, $just:expr, $sev:ident $(,)?) => {
        $crate::_toml_rule!(inventory, $name, $desc, $just, $sev, None);
    };
    ($name:ident, $desc:expr, $just:expr $(,)?) => {
        $crate::_toml_rule!(inventory, $name, $desc, $just, Low, None);
    };
}

/// Verify fix functions resolve all violations in failing examples.
///
/// Use this inside a `rulewright_*_test!` body, which provides the fixture's `run` function.
#[macro_export]
#[doc(hidden)]
macro_rules! fix_tests {
    ($examples:expr, line, $check_fn:ident, $fix_fn:ident) => {
        $crate::__paste::paste! {
            #[cfg(test)]
            #[$crate::_private::gtest]
            fn [<fixes_ $check_fn>]() -> $crate::_private::TestResult {
                for (i, ex) in $examples.iter().enumerate() {
                    if ex.pass {
                        continue;
                    }
                    let fixed = $crate::testing::apply_line_fixes(ex.code, $check_fn, $fix_fn);
                    let remaining = run(&fixed);
                    $crate::_private::scoped_trace!("example {i} ({:?})", ex.label);
                    $crate::_private::verify_true!(remaining.is_empty())?;
                }

                Ok(())
            }
        }
    };
    ($examples:expr, ast, $check_fn:ident, $fix_fn:ident) => {
        $crate::__paste::paste! {
            #[cfg(test)]
            #[$crate::_private::gtest]
            fn [<fixes_ $check_fn>]() -> $crate::_private::TestResult {
                for (i, ex) in $examples.iter().enumerate() {
                    if ex.pass {
                        continue;
                    }
                    let fixed = $crate::testing::apply_ast_fixes(ex.code, $check_fn, $fix_fn);
                    let remaining = run(&fixed);
                    $crate::_private::scoped_trace!("example {i} ({:?})", ex.label);
                    $crate::_private::verify_true!(remaining.is_empty())?;
                }

                Ok(())
            }
        }
    };
    ($examples:expr, ast_tree, $check_fn:ident, $fix_fn:ident) => {
        $crate::__paste::paste! {
            #[cfg(test)]
            #[$crate::_private::gtest]
            fn [<fixes_ $check_fn>]() -> $crate::_private::TestResult {
                for (i, ex) in $examples.iter().enumerate() {
                    if ex.pass {
                        continue;
                    }
                    let fixed = $crate::testing::apply_ast_tree_fix(ex.code, $check_fn, $fix_fn);
                    let remaining = run(&fixed);
                    $crate::_private::scoped_trace!("example {i} ({:?})", ex.label);
                    $crate::_private::verify_true!(remaining.is_empty())?;
                }

                Ok(())
            }
        }
    };
    ($examples:expr, toml, $check_fn:ident, $fix_fn:ident) => {
        $crate::__paste::paste! {
            #[cfg(test)]
            #[$crate::_private::gtest]
            fn [<fixes_ $check_fn>]() -> $crate::_private::TestResult {
                for (i, ex) in $examples.iter().enumerate() {
                    if ex.pass {
                        continue;
                    }
                    let fixed = $crate::testing::apply_toml_fixes(ex.code, $check_fn, $fix_fn);
                    let remaining = run(&fixed);
                    $crate::_private::scoped_trace!("example {i} ({:?})", ex.label);
                    $crate::_private::verify_true!(remaining.is_empty())?;
                }

                Ok(())
            }
        }
    };
    ($examples:expr, toml_at, $rel:expr, $check_fn:ident, $fix_fn:ident) => {
        $crate::__paste::paste! {
            #[cfg(test)]
            #[$crate::_private::gtest]
            fn [<fixes_ $check_fn>]() -> $crate::_private::TestResult {
                for (i, ex) in $examples.iter().enumerate() {
                    if ex.pass {
                        continue;
                    }
                    let fixed =
                        $crate::testing::apply_toml_fixes_at($rel, ex.code, $check_fn, $fix_fn);
                    let remaining = run(&fixed);
                    $crate::_private::scoped_trace!("example {i} ({:?})", ex.label);
                    $crate::_private::verify_true!(remaining.is_empty())?;
                }

                Ok(())
            }
        }
    };
}
