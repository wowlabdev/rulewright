use ra_ap_syntax::ast::{self, HasName};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "few params",
        code: "fn f(a: i32, b: i32) {}",
        pass: true,
    },
    Example {
        label: "too many params",
        code: "fn f(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32) {}",
        pass: false,
    },
    Example {
        label: "self not counted",
        code: "struct S;\nimpl S {\n    fn f(&self, a: i32, b: i32, c: i32, d: i32, e: i32, f: i32) {}\n}",
        pass: true,
    },
    Example {
        label: "at threshold",
        code: "fn f(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32) {}",
        pass: true,
    },
];

crate::ast_rule!(
    large_fn_params,
    "Flag functions with > threshold parameters.",
    "Functions with many parameters are hard to call correctly. Group related params into a struct.",
    Medium,
    params { threshold: i64 = 6 },
);

fn check_large_fn_params(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let max_params = ctx
        .file
        .config
        .get_usize("rust_large_fn_params", &PARAMS[0]);

    ctx.nodes::<ast::Fn>()
        .filter(|function| !ctx.is_in_test(function))
        .filter_map(|function| {
            let count = function.param_list()?.params().count();

            (count > max_params).then(|| {
                let name = function.name()?;

                Some(ctx.violation(
                    &name,
                    format!("function `{name}` has {count} parameters (max {max_params})"),
                ))
            })?
        })
        .collect()
}

crate::rulewright_ast_test!(check_large_fn_params, {
    crate::example_tests!(EXAMPLES, check_large_fn_params);
});
