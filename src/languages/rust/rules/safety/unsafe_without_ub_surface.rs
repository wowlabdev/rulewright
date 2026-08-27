use ra_ap_syntax::{
    AstNode,
    ast::{self, HasAttrs, HasName, UnaryOp},
};

use super::super::support::is_item_or_impl_fn;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "unsafe as danger marker",
        code: "unsafe fn delete_everything(name: &str) { let _ = name; }",
        pass: false,
    },
    Example {
        label: "unsafe method without UB surface",
        code: "struct S;\nimpl S {\n    pub unsafe fn clear(&mut self) {}\n}",
        pass: false,
    },
    Example {
        label: "raw pointer parameter",
        code: "unsafe fn read_at(p: *const u8) -> u8 { *p }",
        pass: true,
    },
    Example {
        label: "NonNull parameter",
        code: "unsafe fn touch(p: std::ptr::NonNull<u8>) { let _ = p; }",
        pass: true,
    },
    Example {
        label: "raw pointer return type",
        code: "unsafe fn alloc_raw() -> *mut u8 { std::ptr::null_mut() }",
        pass: true,
    },
    Example {
        label: "unsafe block in body",
        code: "unsafe fn call() { unsafe { std::ptr::null::<u8>().read(); } }",
        pass: true,
    },
    Example {
        label: "extern abi is exempt",
        code: "unsafe extern \"C\" fn callback() {}",
        pass: true,
    },
    Example {
        label: "no_mangle is exempt",
        code: "#[no_mangle]\npub unsafe fn hook() {}",
        pass: true,
    },
    Example {
        label: "safe fn",
        code: "fn f(x: u32) -> u32 { x }",
        pass: true,
    },
];

crate::ast_rule!(
    unsafe_without_ub_surface,
    "Flag `unsafe fn` with no raw-pointer surface and no unsafe operations in the body.",
    "`unsafe` may only mark undefined-behavior risk, not general danger — a fn without UB surface trains callers to ignore the keyword.",
    Low,
);

fn check_unsafe_without_ub_surface(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let unsafe_functions = ctx
        .nodes::<ast::Fn>()
        .filter(is_item_or_impl_fn)
        .filter(|function| {
            !ctx.is_in_test(function)
                && function.unsafe_token().is_some()
                && function.abi().is_none()
                && !is_no_mangle(function)
        });

    unsafe_functions
        .filter(|function| {
            !signature_has_ub_surface(function)
                && function
                    .body()
                    .is_none_or(|body| !body_has_ub_marker(&body))
        })
        .filter_map(|function| {
            let name = function.name()?;

            Some(ctx.violation(
                &name,
                format!(
                    "unsafe fn `{}` has no UB surface (no raw pointers, no unsafe operations) — \
                     `unsafe` marks UB risk, not general danger",
                    name.text()
                ),
            ))
        })
        .collect()
}

fn is_no_mangle(function: &ast::Fn) -> bool {
    function
        .attrs()
        .any(|attr| attr.syntax().text().to_string().contains("no_mangle"))
}

fn signature_has_ub_surface(function: &ast::Fn) -> bool {
    let parameter_surface = function.param_list().is_some_and(|parameters| {
        parameters
            .self_param()
            .and_then(|parameter| parameter.ty())
            .is_some_and(|ty| type_has_ub_surface(&ty))
            || parameters
                .params()
                .filter_map(|parameter| parameter.ty())
                .any(|ty| type_has_ub_surface(&ty))
    });

    parameter_surface
        || function
            .ret_type()
            .and_then(|ret| ret.ty())
            .is_some_and(|ty| type_has_ub_surface(&ty))
}

fn type_has_ub_surface(ty: &ast::Type) -> bool {
    let rendered = ty.syntax().text().to_string();

    rendered.contains('*') || rendered.contains("NonNull")
}

fn body_has_ub_marker(block: &ast::BlockExpr) -> bool {
    block
        .syntax()
        .descendants()
        .filter_map(ast::PrefixExpr::cast)
        .any(|expr| matches!(expr.op_kind(), Some(UnaryOp::Deref)))
        || block
            .syntax()
            .descendants()
            .filter_map(ast::BlockExpr::cast)
            .any(|block| block.unsafe_token().is_some())
}

crate::rulewright_ast_test!(check_unsafe_without_ub_surface, {
    crate::example_tests!(EXAMPLES, check_unsafe_without_ub_surface);
});
