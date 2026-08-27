use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName, HasVisibility, VisibilityKind},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "transmute in safe pub fn",
        code: "pub fn f(x: u32) -> f32 { unsafe { std::mem::transmute(x) } }",
        pass: false,
    },
    Example {
        label: "transmute in safe pub method",
        code: "struct S;\nimpl S {\n    pub fn f(&self, x: u32) -> f32 { unsafe { std::mem::transmute(x) } }\n}",
        pass: false,
    },
    Example {
        label: "transmute in pub unsafe fn",
        code: "pub unsafe fn f(x: u32) -> f32 { unsafe { std::mem::transmute(x) } }",
        pass: true,
    },
    Example {
        label: "transmute in private fn",
        code: "fn f(x: u32) -> f32 { unsafe { std::mem::transmute(x) } }",
        pass: true,
    },
    Example {
        label: "transmute in pub(crate) fn",
        code: "pub(crate) fn f(x: u32) -> f32 { unsafe { std::mem::transmute(x) } }",
        pass: true,
    },
    Example {
        label: "transmute in test module",
        code: "#[cfg(test)]\nmod tests {\n    pub fn t(x: u32) -> f32 { unsafe { std::mem::transmute(x) } }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    transmute_in_safe_fn,
    "Flag `transmute` inside a safe `pub` fn.",
    "A safe public signature promises soundness its transmuting body cannot guarantee — the prime unsoundness suspect.",
    High,
);

fn check_transmute_in_safe_fn(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::CallExpr>()
        .filter(|call| !ctx.is_in_test(call))
        .filter_map(|call| {
            let ast::Expr::PathExpr(path_expr) = call.expr()? else {
                return None;
            };
            let path = path_expr.path()?;

            if path
                .segment()
                .and_then(|segment| segment.name_ref()).is_none_or(|name| name.text() != "transmute")
            {
                return None;
            }

            let function = call.syntax().ancestors().find_map(ast::Fn::cast)?;
            let name = safe_pub_fn_name(&function)?;

            Some(ctx.violation(
                &path,
                format!(
                    "transmute inside safe `pub fn {name}` — mark it `unsafe fn` or encapsulate the invariant"
                ),
            ))
        })
        .collect()
}

fn safe_pub_fn_name(function: &ast::Fn) -> Option<String> {
    (function
        .visibility()
        .is_some_and(|visibility| matches!(visibility.kind(), VisibilityKind::Pub))
        && function.unsafe_token().is_none())
    .then(|| function.name().map(|name| name.text().to_string()))
    .flatten()
}

crate::rulewright_ast_test!(check_transmute_in_safe_fn, {
    crate::example_tests!(EXAMPLES, check_transmute_in_safe_fn);
});
