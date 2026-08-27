use ra_ap_syntax::{
    ast,
    ast::{HasGenericArgs, HasName},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "Result with unit error",
        code: "fn f() -> Result<i32, ()> { Ok(1) }",
        pass: false,
    },
    Example {
        label: "Result with named error",
        code: "fn f() -> Result<i32, MyError> { Ok(1) }",
        pass: true,
    },
    Example {
        label: "no return type",
        code: "fn f() { }",
        pass: true,
    },
    Example {
        label: "non-Result return",
        code: "fn f() -> i32 { 1 }",
        pass: true,
    },
    Example {
        label: "unit error in test module",
        code: "#[cfg(test)]\nmod tests {\n  fn f() -> Result<i32, ()> { Ok(1) }\n}",
        pass: true,
    },
    Example {
        label: "impl fn with unit error",
        code: "struct S;\nimpl S {\n  fn f(&self) -> Result<(), ()> { Ok(()) }\n}",
        pass: false,
    },
];

crate::ast_rule!(
    error_type_unit,
    "Flag `Result<T, ()>` return types — use a real error type.",
    "Result<T, ()> discards error information. Use a real error type so callers can diagnose failures.",
    Medium,
);

fn check_error_type_unit(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let result_functions = ctx
        .nodes::<ast::Fn>()
        .filter(|function| !ctx.is_in_test(function))
        .filter(|function| {
            function
                .ret_type()
                .and_then(|ret| ret.ty())
                .is_some_and(|ty| has_result_unit_error(&ty))
        });

    result_functions
        .filter_map(|function| {
            let name = function.name()?;

            Some(ctx.violation(
                &name,
                format!("function `{name}` returns Result<_, ()> — use a meaningful error type"),
            ))
        })
        .collect()
}

fn has_result_unit_error(ty: &ast::Type) -> bool {
    let ast::Type::PathType(type_path) = ty else {
        return false;
    };
    let Some(segment) = type_path.path().and_then(|path| path.segment()) else {
        return false;
    };

    if segment
        .name_ref()
        .is_none_or(|name| name.text() != "Result")
    {
        return false;
    }

    let Some(error_type) = segment
        .generic_arg_list()
        .and_then(|args| args.generic_args().nth(1))
        .and_then(|arg| match arg {
            ast::GenericArg::TypeArg(arg) => arg.ty(),
            _ => None,
        })
    else {
        return false;
    };

    matches!(error_type, ast::Type::TupleType(tuple) if tuple.fields().next().is_none())
}

crate::rulewright_ast_test!(check_error_type_unit, {
    crate::example_tests!(EXAMPLES, check_error_type_unit);
});
