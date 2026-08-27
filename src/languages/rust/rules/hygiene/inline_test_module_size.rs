#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasAttrs, HasName},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "small test module",
        code: "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}",
        pass: true,
    },
    Example {
        label: "non-test module",
        code: "mod helpers {\n    fn h() {}\n}",
        pass: true,
    },
];

crate::ast_rule!(
    inline_test_module_size,
    "Flag `#[cfg(test)] mod` blocks spanning more than threshold lines.",
    "Oversized inline test modules drown the business logic in the same file; tests touching only public API are integration tests and belong under `tests/`.",
    Low,
    params {
        threshold: i64 = 200
    },
);

fn check_inline_test_module_size(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let threshold = ctx
        .file
        .config
        .get_usize("rust_inline_test_module_size", &PARAMS[0]);

    ctx.nodes::<ast::Module>()
        .filter(is_cfg_test_module)
        .filter_map(|module| {
            let item_list = module.item_list()?;
            let range = item_list.syntax().text_range();
            let start = ctx.line_index.line_col(range.start()).line as usize + 1;
            let end = ctx.line_index.line_col(range.end()).line as usize + 1;
            let lines = end.saturating_sub(start);

            if lines <= threshold {
                return None;
            }

            let name = module.name()?;

            Some(ctx.violation(
                &name,
                format!(
                    "#[cfg(test)] mod `{name}` is {lines} lines long (max {threshold}) — move public-API tests under tests/"
                ),
            ))
        })
        .collect()
}

fn is_cfg_test_module(module: &ast::Module) -> bool {
    module.attrs().any(|attr| {
        attr.simple_name().is_some_and(|name| name == "cfg")
            && attr
                .syntax()
                .descendants_with_tokens()
                .filter_map(ra_ap_syntax::NodeOrToken::into_token)
                .any(|token| token.text() == "test")
    })
}

crate::rulewright_ast_test!(check_inline_test_module_size, {
    use std::fmt::Write as _;

    crate::example_tests!(EXAMPLES, check_inline_test_module_size);

    fn module_with_lines(attr: &str, count: usize) -> String {
        let mut src = format!("{attr}\nmod tests {{\n");
        for i in 0..count {
            let _ = writeln!(src, "    fn t{i}() {{}}");
        }
        src.push_str("}\n");
        src
    }

    #[gtest]
    fn oversized_test_module_fails() -> Result<()> {
        let v = run(&module_with_lines("#[cfg(test)]", 205));
        verify_eq!(v.len(), 1)?;
        verify_true!(v[0].message.contains("tests"))?;

        Ok(())
    }

    #[gtest]
    fn oversized_plain_module_passes() -> Result<()> {
        let v = run(&module_with_lines("#[rustfmt::skip]", 205));
        verify_true!(v.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn test_module_at_threshold_passes() -> Result<()> {
        let v = run(&module_with_lines("#[cfg(test)]", 150));
        verify_true!(v.is_empty())?;

        Ok(())
    }
});
