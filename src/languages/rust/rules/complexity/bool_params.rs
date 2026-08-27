use ra_ap_syntax::ast::{self, HasName};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "two bool params",
        code: "fn f(a: bool, b: bool) {}",
        pass: false,
    },
    Example {
        label: "one bool param",
        code: "fn f(a: bool, b: i32) {}",
        pass: true,
    },
    Example {
        label: "no bool params",
        code: "fn f(a: i32, b: i32) {}",
        pass: true,
    },
    Example {
        label: "self not counted",
        code: "struct S;\nimpl S {\n  fn f(&self, a: bool) {}\n}",
        pass: true,
    },
    Example {
        label: "three bool params",
        code: "fn f(a: bool, b: bool, c: bool) {}",
        pass: false,
    },
    Example {
        label: "bool params in test module",
        code: "#[cfg(test)]\nmod tests {\n  fn f(a: bool, b: bool) {}\n}",
        pass: true,
    },
];

crate::ast_rule!(
    bool_params,
    "Flag functions with threshold+ `bool` parameters (error-prone API design).",
    "Multiple bool parameters are easy to mix up at call sites. Use an enum to make each argument self-documenting.",
    Medium,
    params { threshold: i64 = 2 },
);

fn check_bool_params(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let threshold = ctx.file.config.get_usize("rust_bool_params", &PARAMS[0]);

    ctx.nodes::<ast::Fn>()
        .filter(|function| !ctx.is_in_test(function))
        .filter_map(|function| {
            let bool_count = function
                .param_list()?
                .params()
                .filter_map(|parameter| parameter.ty())
                .filter(is_bool_type)
                .count();

            (bool_count >= threshold).then(|| {
                let name = function.name()?;

                Some(ctx.violation(
                    &name,
                    format!(
                        "function `{name}` has {bool_count} bool parameters — consider using an enum"
                    ),
                ))
            })?
        })
        .collect()
}

fn is_bool_type(ty: &ast::Type) -> bool {
    let ast::Type::PathType(path_type) = ty else {
        return false;
    };
    let Some(path) = path_type.path() else {
        return false;
    };

    path.qualifier().is_none()
        && path
            .segment()
            .and_then(|segment| segment.name_ref())
            .is_some_and(|name| name.text() == "bool")
}

crate::rulewright_ast_test!(check_bool_params, {
    crate::example_tests!(EXAMPLES, check_bool_params);
});
