use ra_ap_syntax::ast::{self, HasAttrs, HasDocComments, HasName, LiteralKind};

use super::super::support::is_item_or_impl_fn;
use crate::{AstCtx, Example, Violation, infra::helpers};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "unsafe fn without safety docs",
        code: "unsafe fn f(x: u32) -> u32 { x }",
        pass: false,
    },
    Example {
        label: "private unsafe fn without safety docs",
        code: "pub(crate) unsafe fn f() {}",
        pass: false,
    },
    Example {
        label: "unsafe method without safety docs",
        code: "struct S;\nimpl S {\n    unsafe fn m(&self) {}\n}",
        pass: false,
    },
    Example {
        label: "unsafe fn with Safety doc section",
        code: "/// Reads the value behind `p`.\n///\n/// # Safety\n/// `p` must be valid and aligned.\nunsafe fn f(p: *const u8) -> u8 { unsafe { *p } }",
        pass: true,
    },
    Example {
        label: "unsafe fn with SAFETY comment",
        code: "// SAFETY: callers uphold the invariants documented on the module\nunsafe fn f() {}",
        pass: true,
    },
    Example {
        label: "safe fn needs nothing",
        code: "fn f(x: u32) -> u32 { x }",
        pass: true,
    },
    Example {
        label: "extern block fns are exempt",
        code: "extern \"C\" {\n    fn ffi(x: u32) -> u32;\n}",
        pass: true,
    },
    Example {
        label: "unsafe fn in test module",
        code: "#[cfg(test)]\nmod tests {\n    unsafe fn t() {}\n}",
        pass: true,
    },
];

crate::ast_rule!(
    unsafe_fn_safety_doc,
    "Require a `# Safety` doc section or `// SAFETY:` comment on every `unsafe fn`.",
    "Callers cannot uphold contracts that are not written down, and clippy's missing_safety_doc only covers public functions.",
    High,
);

fn check_unsafe_fn_safety_doc(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let unsafe_functions = ctx
        .nodes::<ast::Fn>()
        .filter(is_item_or_impl_fn)
        .filter(|function| function.unsafe_token().is_some() && !ctx.is_in_test(function))
        .filter(|function| !has_safety_heading(function));

    unsafe_functions
        .filter_map(|function| {
            let line = function.attrs().next().map_or_else(
                || {
                    function.fn_token().map_or_else(
                        || ctx.line_of(&function),
                        |token| {
                            ctx.line_index.line_col(token.text_range().start()).line as usize + 1
                        },
                    )
                },
                |attr| ctx.line_of(&attr),
            );

            if helpers::has_preceding_comment(ctx.file.lines, line, &["SAFETY:"]) {
                return None;
            }

            let name = function.name()?;

            Some(ctx.violation(
                &name,
                format!(
                    "unsafe fn `{}` lacks both a `# Safety` doc section and a `// SAFETY:` comment",
                    name.text()
                ),
            ))
        })
        .collect()
}

fn has_safety_heading(function: &ast::Fn) -> bool {
    function.doc_comments().any(|comment| {
        comment
            .doc_comment()
            .is_some_and(|(text, _)| text.contains("# Safety"))
    }) || function.attrs().any(|attr| {
        let Some(ast::Meta::KeyValueMeta(meta)) = attr.meta() else {
            return false;
        };
        let name = meta.path()
            .and_then(|path| path.segment())
            .and_then(|segment| segment.name_ref());
        let is_doc = name.is_some_and(|name| name.text() == "doc");
        let Some(ast::Expr::Literal(literal)) = meta.expr() else {
            return false;
        };

        is_doc
            && matches!(literal.kind(), LiteralKind::String(text) if text.value().is_ok_and(|value| value.contains("# Safety")))
    })
}

crate::rulewright_ast_test!(check_unsafe_fn_safety_doc, {
    crate::example_tests!(EXAMPLES, check_unsafe_fn_safety_doc);
});
