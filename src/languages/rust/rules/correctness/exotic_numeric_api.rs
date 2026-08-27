use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName, HasVisibility, VisibilityKind},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "pub fn NonZero param",
        code: "pub fn window(n: std::num::NonZeroUsize) -> usize { n.get() }",
        pass: false,
    },
    Example {
        label: "pub fn Wrapping return",
        code: "pub fn count() -> std::num::Wrapping<u32> { std::num::Wrapping(0) }",
        pass: false,
    },
    Example {
        label: "pub fn Saturating param",
        code: "pub fn add(x: std::num::Saturating<u32>) -> u32 { x.0 }",
        pass: false,
    },
    Example {
        label: "pub method NonZero param",
        code: "struct S;\nimpl S {\n    pub fn set(&self, n: std::num::NonZeroU8) -> u8 { n.get() }\n}",
        pass: false,
    },
    Example {
        label: "private fn NonZero param",
        code: "fn helper(n: std::num::NonZeroU8) -> u8 { n.get() }",
        pass: true,
    },
    Example {
        label: "pub fn plain numbers",
        code: "pub fn window_size(n: usize) -> usize { n }",
        pass: true,
    },
    Example {
        label: "exotic type only inside body",
        code: "pub fn f(n: u32) -> u32 { let w = std::num::Wrapping(n); w.0 }",
        pass: true,
    },
    Example {
        label: "pub fn in test module",
        code: "#[cfg(test)]\nmod tests {\n    pub fn window(n: std::num::NonZeroUsize) -> usize { n.get() }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    exotic_numeric_api,
    "Flag `Saturating`/`Wrapping`/`NonZero*` in pub fn signatures.",
    "Std convention is plain numbers at public numeric boundaries; exotic wrappers belong to internal arithmetic.",
    Low,
);

fn exotic_numeric_name(ty: &ast::Type) -> Option<String> {
    ty.syntax()
        .descendants()
        .filter_map(ast::NameRef::cast)
        .map(|name| name.text().to_string())
        .find(|name| name == "Saturating" || name == "Wrapping" || name.starts_with("NonZero"))
}

fn check_exotic_numeric_api(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let mut violations = Vec::new();

    for function in ctx.nodes::<ast::Fn>().filter(|function| {
        !ctx.is_in_test(function)
            && function
                .visibility()
                .is_some_and(|visibility| matches!(visibility.kind(), VisibilityKind::Pub))
    }) {
        let Some(function_name) = function.name() else {
            continue;
        };
        let parameter_types = function
            .param_list()
            .into_iter()
            .flat_map(|parameters| parameters.params())
            .filter_map(|parameter| parameter.ty());
        let return_type = function.ret_type().and_then(|ret| ret.ty());

        for ty in parameter_types.chain(return_type) {
            if let Some(name) = exotic_numeric_name(&ty) {
                violations.push(ctx.violation(
                    &function_name,
                    format!(
                        "pub fn `{function_name}` uses `{name}` in its signature — public numeric boundaries take plain numbers"
                    ),
                ));
            }
        }
    }

    violations
}

crate::rulewright_ast_test!(check_exotic_numeric_api, {
    crate::example_tests!(EXAMPLES, check_exotic_numeric_api);
});
