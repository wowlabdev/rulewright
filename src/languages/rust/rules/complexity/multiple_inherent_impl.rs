// #rw(file: rust_default_hasher) small per-file dedup set; fast-hasher dependency not warranted

use std::collections::HashSet;

#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::ast;

use super::support;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "duplicate impl blocks",
        code: "struct Foo;\nimpl Foo { fn a() {} }\nimpl Foo { fn b() {} }",
        pass: false,
    },
    Example {
        label: "different types",
        code: "struct Foo;\nstruct Bar;\nimpl Foo {}\nimpl Bar {}",
        pass: true,
    },
    Example {
        label: "trait impl not counted",
        code: "struct Foo;\nimpl Foo {}\nimpl std::fmt::Display for Foo { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { Ok(()) } }",
        pass: true,
    },
    Example {
        label: "duplicate impl in test module",
        code: "#[cfg(test)]\nmod tests {\n  struct Foo;\n  impl Foo { fn a() {} }\n  impl Foo { fn b() {} }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    multiple_inherent_impl,
    "Flag multiple `impl Foo` blocks for the same type in one file.",
    "Split impl blocks for the same type can scatter related methods. Merge blocks that form one readable API inventory; suppress the rule when cfg boundaries, generated code, or a deliberate large-module organization makes separation clearer.",
);

fn check_multiple_inherent_impl(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let mut seen = HashSet::new();
    let mut violations = Vec::new();

    for item_impl in ctx
        .nodes::<ast::Impl>()
        .filter(|item_impl| !ctx.is_in_test(item_impl) && item_impl.trait_().is_none())
    {
        let Some(self_type) = item_impl.self_ty() else {
            continue;
        };
        let Some(name) = support::self_type_name(&self_type) else {
            continue;
        };

        if seen.contains(&name) {
            violations.push(ctx.violation(
                &self_type,
                format!("multiple inherent impl blocks for `{name}` in the same file — merge them"),
            ));
        } else {
            seen.insert(name);
        }
    }

    violations
}

crate::rulewright_ast_test!(check_multiple_inherent_impl, {
    crate::example_tests!(EXAMPLES, check_multiple_inherent_impl);

    #[gtest]
    fn three_impls_flags_second_and_third() -> Result<()> {
        let v = run("struct Foo;\nimpl Foo {}\nimpl Foo {}\nimpl Foo {}");
        verify_eq!(v.len(), 2)?;

        Ok(())
    }
});
