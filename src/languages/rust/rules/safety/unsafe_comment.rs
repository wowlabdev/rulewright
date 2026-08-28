use ra_ap_syntax::ast::{self};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "unsafe without safety comment",
        code: "fn f() { unsafe { std::ptr::null::<u8>().read() }; }",
        pass: false,
    },
    Example {
        label: "unsafe with safety comment",
        code: "fn f() {\n    // SAFETY: pointer is valid and aligned\n    unsafe { std::ptr::null::<u8>().read() };\n}",
        pass: true,
    },
    Example {
        label: "unsafe in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn t() { unsafe { std::ptr::null::<u8>().read() }; }\n}",
        pass: true,
    },
    Example {
        label: "safety comment with blank line",
        code: "fn f() {\n    // SAFETY: guaranteed valid\n\n    unsafe { std::ptr::null::<u8>().read() };\n}",
        pass: true,
    },
];

crate::ast_rule!(
    unsafe_comment,
    "Require `// SAFETY:` comment on `unsafe` blocks.",
    "Every unsafe block must expose its proof obligation to reviewers. Put a SAFETY comment directly above it that names the concrete validity, aliasing, lifetime, or synchronization invariant; generic claims that the code is safe are not evidence.",
    High,
);

fn check_unsafe_comment(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::BlockExpr>()
        .filter(|block| block.unsafe_token().is_some() && !ctx.is_in_test(block))
        .filter_map(|block| {
            let unsafe_token = block.unsafe_token()?;
            let line = ctx
                .line_index
                .line_col(unsafe_token.text_range().start())
                .line as usize
                + 1;

            (!crate::infra::helpers::has_preceding_comment(ctx.file.lines, line, &["SAFETY:"]))
                .then(|| {
                    ctx.violation(
                        &block,
                        "unsafe block without // SAFETY: comment on a preceding line",
                    )
                })
        })
        .collect()
}

crate::rulewright_ast_test!(check_unsafe_comment, {
    crate::example_tests!(EXAMPLES, check_unsafe_comment);
});
