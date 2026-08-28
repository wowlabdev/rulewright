use ra_ap_syntax::ast;

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "bare transmute without safety comment",
        code: "unsafe fn f() { let _x: u32 = std::mem::transmute(1.0f32); }",
        pass: false,
    },
    Example {
        label: "transmute with safety comment",
        code: "unsafe fn f() {\n    // SAFETY: f32 and u32 have the same size\n    let _x: u32 = std::mem::transmute(1.0f32);\n}",
        pass: true,
    },
    Example {
        label: "transmute in test module",
        code: "#[cfg(test)]\nmod tests {\n    unsafe fn t() { let _x: u32 = std::mem::transmute(1.0f32); }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    transmute_usage,
    "Require `SAFETY` comment on `std::mem::transmute` calls.",
    "transmute can violate size, validity, provenance, and lifetime rules. Prefer a checked conversion or dedicated pointer API; if transmute is necessary, document the exact invariants that make the source and destination representations valid.",
    High,
);

fn check_transmute_usage(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::CallExpr>()
        .filter(|call| !ctx.is_in_test(call))
        .filter_map(|call| {
            let ast::Expr::PathExpr(path_expr) = call.expr()? else {
                return None;
            };
            let path = path_expr.path()?;

            if path
                .segment()
                .and_then(|segment| segment.name_ref())
                .is_none_or(|name| name.text() != "transmute")
            {
                return None;
            }

            let line = ctx.line_of(&path);

            (!crate::infra::helpers::has_preceding_comment(ctx.file.lines, line, &["SAFETY:"]))
                .then(|| {
                    ctx.violation(
                        &path,
                        "std::mem::transmute without // SAFETY: comment on a preceding line",
                    )
                })
        })
        .collect()
}

crate::rulewright_ast_test!(check_transmute_usage, {
    crate::example_tests!(EXAMPLES, check_transmute_usage);
});
