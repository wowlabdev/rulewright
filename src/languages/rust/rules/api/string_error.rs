use ra_ap_syntax::{
    ast,
    ast::{HasGenericArgs, HasName},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example { label: "owned string error", code: "fn parse() -> Result<u32, String> { Err(String::new()) }", pass: false },
    Example { label: "borrowed string error", code: "fn parse() -> Result<u32, &'static str> { Err(\"bad\") }", pass: false },
    Example { label: "qualified result", code: "fn parse() -> std::result::Result<u32, String> { Err(String::new()) }", pass: false },
    Example { label: "typed error", code: "fn parse() -> Result<u32, ParseError> { todo!() }", pass: true },
    Example { label: "string success", code: "fn parse() -> Result<String, ParseError> { todo!() }", pass: true },
    Example { label: "test helper", code: "#[cfg(test)] mod tests { fn parse() -> Result<u32, String> { Err(String::new()) } }", pass: true },
];

crate::ast_rule!(
    string_error,
    "Reject `String` and `&str` as function error types.",
    "Structured error types preserve context, support source chains, and give callers stable inspection APIs.",
    Medium,
);

fn check_string_error(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::Fn>()
        .filter(|function| !ctx.is_in_test(function))
        .filter_map(|function| {
            let return_type = function.ret_type()?.ty()?;

            string_error_type(&return_type).then(|| {
                let name = function
                    .name()
                    .map_or_else(|| "<anonymous>".to_owned(), |name| name.text().to_string());

                ctx.violation(
                    &return_type,
                    format!(
                        "function `{name}` returns an unstructured string error — use a canonical error type"
                    ),
                )
            })
        })
        .collect()
}

fn string_error_type(ty: &ast::Type) -> bool {
    let ast::Type::PathType(path_type) = ty else {
        return false;
    };
    let Some(segment) = path_type.path().and_then(|path| path.segment()) else {
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

    match error_type {
        ast::Type::PathType(path_type) => {
            let segment = path_type
                .path()
                .and_then(|path| path.segment())
                .and_then(|segment| segment.name_ref());

            segment.is_some_and(|name| name.text() == "String")
        }
        ast::Type::RefType(reference) => {
            let path = reference
                .ty()
                .and_then(|ty| match ty {
                    ast::Type::PathType(path_type) => path_type.path(),
                    _ => None,
                })
                .and_then(|path| path.segment());

            path.and_then(|segment| segment.name_ref())
                .is_some_and(|name| name.text() == "str")
        }
        _ => false,
    }
}

crate::rulewright_ast_test!(check_string_error, {
    crate::example_tests!(EXAMPLES, check_string_error);
});
