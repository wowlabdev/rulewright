// #rw(file: rust_default_hasher) small per-file type map; hashing and preallocation are not bottlenecks

use std::collections::HashMap;

use ra_ap_syntax::{AstNode, ast, ast::HasName};

use super::super::support::type_name;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "impl before struct",
        code: "impl Foo { fn a() {} }\nstruct Foo;",
        pass: false,
    },
    Example {
        label: "struct before impl",
        code: "struct Foo;\nimpl Foo { fn a() {} }",
        pass: true,
    },
    Example {
        label: "trait impl before struct",
        code: "impl std::fmt::Display for Foo { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { Ok(()) } }\nstruct Foo;",
        pass: true,
    },
    Example {
        label: "impl for external type",
        code: "impl Foo { fn a() {} }",
        pass: true,
    },
    Example {
        label: "enum before impl",
        code: "enum Color { Red, Blue }\nimpl Color { fn name(&self) -> &str { \"\" } }",
        pass: true,
    },
    Example {
        label: "impl before enum",
        code: "impl Color { fn name(&self) -> &str { \"\" } }\nenum Color { Red, Blue }",
        pass: false,
    },
    Example {
        label: "impl before struct in test module",
        code: "#[cfg(test)]\nmod tests {\n    impl Foo { fn a() {} }\n    struct Foo;\n}",
        pass: true,
    },
];

crate::ast_rule!(
    type_def_ordering,
    "Flag `impl` blocks that appear before their type definition.",
    "Reading code top-down, you expect to see a type defined before its methods. Impl-before-struct breaks that flow.",
);

fn check_type_def_ordering(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let mut violations = Vec::new();

    let items: Vec<ast::Item> = ctx
        .root
        .syntax()
        .children()
        .filter_map(ast::Item::cast)
        .collect();
    let type_lines: HashMap<String, usize> = items
        .iter()
        .filter(|item| !ctx.is_in_test(*item))
        .filter_map(|item| match item {
            ast::Item::Struct(item) => item
                .name()
                .map(|name| (name.text().to_string(), ctx.line_of(&name))),

            ast::Item::Enum(item) => item
                .name()
                .map(|name| (name.text().to_string(), ctx.line_of(&name))),
            _ => None,
        })
        .collect();

    for item in items {
        if ctx.is_in_test(&item) {
            continue;
        }

        if let ast::Item::Impl(item_impl) = item {
            if item_impl.trait_().is_some() {
                continue;
            }

            if let Some(self_type) = item_impl.self_ty()
                && let Some(name) = type_name(&self_type)
                && let Some(&def_line) = type_lines.get(&name)
            {
                let impl_line = ctx.line_of(&self_type);

                if impl_line < def_line {
                    violations.push(ctx.violation(
                        &self_type,
                        format!(
                            "`impl {name}` appears before the type definition — move the impl after the type"
                        ),
                    ));
                }
            }
        }
    }

    violations
}

crate::rulewright_ast_test!(check_type_def_ordering, {
    crate::example_tests!(EXAMPLES, check_type_def_ordering);
});
