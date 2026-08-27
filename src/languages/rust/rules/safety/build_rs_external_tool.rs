#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasArgList, LiteralKind},
};

use super::support::path_segments;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "external tool invocation",
        code: "fn main() { let _ = std::process::Command::new(\"cmake\").status(); }",
        pass: false,
    },
    Example {
        label: "compiler driver is allowed",
        code: "fn main() { let _ = std::process::Command::new(\"cc\").status(); }",
        pass: true,
    },
    Example {
        label: "required env var via unwrap",
        code: "fn main() { let _ = std::env::var(\"FOO_LIB_DIR\").unwrap(); }",
        pass: false,
    },
    Example {
        label: "required env var via expect",
        code: "fn main() { let _ = std::env::var(\"FOO_LIB_DIR\").expect(\"FOO_LIB_DIR must be set\"); }",
        pass: false,
    },
    Example {
        label: "required env var via question mark",
        code: "fn main() -> Result<(), std::env::VarError> {\n    let _ = std::env::var(\"FOO_LIB_DIR\")?;\n    Ok(())\n}",
        pass: false,
    },
    Example {
        label: "defaulted env var is fine",
        code: "fn main() { let _ = std::env::var(\"PROFILE\").unwrap_or_default(); }",
        pass: true,
    },
    Example {
        label: "optional env var is fine",
        code: "fn main() { if let Ok(dir) = std::env::var(\"FOO_DIR\") { let _ = dir; } }",
        pass: true,
    },
    Example {
        label: "build-time bindgen",
        code: "fn main() { let _ = bindgen::Builder::default(); }",
        pass: false,
    },
    Example {
        label: "build-time pkg_config probe",
        code: "fn main() { let _ = pkg_config::probe_library(\"foo\"); }",
        pass: false,
    },
    Example {
        label: "plain rerun directive",
        code: "fn main() { println!(\"cargo:rerun-if-changed=src/schema.json\"); }",
        pass: true,
    },
];

crate::ast_rule!(
    build_rs_external_tool,
    "Flag build.rs usage of external tools, hard-required env vars, and build-time binding generation.",
    "Builds must work out of the box with cargo and rustc alone — external tools and required env vars break every downstream consumer.",
    Medium,
);

const ALLOWED_TOOLS: &[&str] = &["rustc", "cargo", "cc", "c++", "clang", "gcc"];
const SYS_BUILD_TOOLS: &[&str] = &["make", "cmake", "sh", "bash", "ninja"];
const BINDING_GENERATORS: &[&str] = &["bindgen", "pkg_config"];
/// An unprefixed `env::var` call is exactly two path segments.
const MODULE_CALL_SEGMENTS: usize = 2;

fn check_build_rs_external_tool(ctx: &AstCtx<'_>) -> Vec<Violation> {
    if !ctx.file.rel.ends_with("build.rs") {
        return Vec::new();
    }

    let is_sys = ctx
        .file
        .rel
        .rsplit('/')
        .nth(1)
        .is_some_and(|dir| dir.ends_with("-sys") || dir.ends_with("_sys"));
    let mut violations = Vec::new();

    for call in ctx
        .nodes::<ast::CallExpr>()
        .filter(|call| !ctx.is_in_test(call))
    {
        check_command_new(ctx, &call, is_sys, &mut violations);
        check_binding_generator_path(ctx, &call, &mut violations);
    }

    violations.extend({
        let unsafe_calls = ctx
            .nodes::<ast::MethodCallExpr>()
            .filter(|call| !ctx.is_in_test(call))
            .filter(|call| {
                call.name_ref()
                    .is_some_and(|name| matches!(name.text().as_str(), "unwrap" | "expect"))
                    && call.receiver().is_some_and(|expr| is_env_var_call(&expr))
            });

        unsafe_calls.map(|call| env_var_violation(ctx, &call))
    });
    violations.extend({
        let unsafe_tries = ctx
            .nodes::<ast::TryExpr>()
            .filter(|expr| !ctx.is_in_test(expr))
            .filter(|expr| expr.expr().is_some_and(|expr| is_env_var_call(&expr)));

        unsafe_tries.map(|expr| env_var_violation(ctx, &expr))
    });

    violations
}

fn check_command_new(
    ctx: &AstCtx<'_>,
    expr: &ast::CallExpr,
    is_sys: bool,
    violations: &mut Vec<Violation>,
) {
    let Some(segments) = call_path_segments(expr) else {
        return;
    };
    let [.., ty, method] = segments.as_slice() else {
        return;
    };

    if ty != "Command" || method != "new" {
        return;
    }

    let Some(tool) = first_arg_str_literal(expr) else {
        return;
    };

    if ALLOWED_TOOLS.contains(&tool.as_str()) {
        return;
    }

    let message = if is_sys && SYS_BUILD_TOOLS.contains(&tool.as_str()) {
        format!(
            "external build system `{tool}` in a -sys crate build.rs — govern the native \
             build with the `cc` crate instead"
        )
    } else {
        format!("external tool `{tool}` invoked from build.rs breaks out-of-the-box builds")
    };

    violations.push(ctx.violation(expr, message));
}

fn check_binding_generator_path(
    ctx: &AstCtx<'_>,
    expr: &ast::CallExpr,
    violations: &mut Vec<Violation>,
) {
    let Some(root) = call_path_segments(expr).and_then(|segments| segments.into_iter().next())
    else {
        return;
    };

    if BINDING_GENERATORS.contains(&root.as_str()) {
        violations.push(ctx.violation(
            expr,
            format!(
                "`{root}` runs at build time — pre-generate the bindings and ship them \
                 in the crate"
            ),
        ));
    }
}

fn env_var_violation<N>(ctx: &AstCtx<'_>, node: &N) -> Violation
where
    N: AstNode,
{
    ctx.violation(
        node,
        "build.rs fails when this env var is absent — default it with `unwrap_or`/`ok()` \
         or make the step optional",
    )
}

fn call_path_segments(expr: &ast::CallExpr) -> Option<Vec<String>> {
    let ast::Expr::PathExpr(path_expr) = expr.expr()? else {
        return None;
    };

    path_expr.path().map(|path| path_segments(&path))
}

fn first_arg_str_literal(expr: &ast::CallExpr) -> Option<String> {
    let ast::Expr::Literal(literal) = expr.arg_list()?.args().next()? else {
        return None;
    };
    let LiteralKind::String(text) = literal.kind() else {
        return None;
    };

    text.value().ok().map(std::borrow::Cow::into_owned)
}

fn is_env_var_call(expr: &ast::Expr) -> bool {
    let ast::Expr::CallExpr(call) = expr else {
        return false;
    };
    let Some(segments) = call_path_segments(call) else {
        return false;
    };
    let [.., module, name] = segments.as_slice() else {
        return false;
    };
    let rooted = segments.len() == MODULE_CALL_SEGMENTS
        || segments.first().is_some_and(|root| root == "std");

    rooted && module == "env" && name == "var"
}

// The default test rel would defeat the build.rs gate, so examples run through the
// rel-aware helper instead of `example_tests!`.
#[cfg(test)]
mod tests {
    use super::*;

    const BUILD_RS_REL: &str = "crates/mylib/build.rs";

    fn run_at(rel: &str, source: &str) -> Vec<Violation> {
        crate::test_support::check_source_ast_at(rel, source, check_build_rs_external_tool)
    }

    #[gtest]
    fn examples() -> Result<()> {
        for ex in EXAMPLES {
            let violations = run_at(BUILD_RS_REL, ex.code);

            verify_eq!(violations.is_empty(), ex.pass)?;
        }

        Ok(())
    }

    #[gtest]
    fn non_build_rs_files_are_exempt() -> Result<()> {
        for ex in EXAMPLES {
            let violations = run_at("crates/mylib/src/lib.rs", ex.code);

            verify_true!(violations.is_empty())?;
        }

        Ok(())
    }

    #[gtest]
    fn sys_crate_build_system_gets_sys_message() -> Result<()> {
        let violations = run_at(
            "crates/foo-sys/build.rs",
            "fn main() { let _ = std::process::Command::new(\"make\").status(); }",
        );

        verify_eq!(violations.len(), 1)?;
        verify_true!(violations[0].message.contains("external build system"))?;

        Ok(())
    }
}
