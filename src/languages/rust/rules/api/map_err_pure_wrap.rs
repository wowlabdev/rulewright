use ra_ap_syntax::{AstNode, ast, ast::HasArgList, ast::HasGenericArgs};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "bare variant path",
        code: "fn f(r: Result<u8, IoError>) -> Result<u8, AppError> { r.map_err(AppError::Io) }",
        pass: true,
    },
    Example {
        label: "explicit conversion immediately propagated",
        code: "fn f(r: Result<u8, IoError>) -> Result<(), AppError> { let _ = r.map_err(<AppError as From<IoError>>::from)?; Ok(()) }",
        pass: false,
    },
    Example {
        label: "inherent conversion does not prove the propagation conversion",
        code: "fn f(r: Result<u8, A>) -> Result<(), C> { let _ = r.map_err(B::from)?; Ok(()) }",
        pass: true,
    },
    Example {
        label: "mapped result retained as a value",
        code: "fn f(r: Result<u8, IoError>) -> bool { let normalized = r.map_err(AppError::from); normalized.is_ok() }",
        pass: true,
    },
    Example {
        label: "mapped result returned without immediate propagation",
        code: "fn f(r: Result<u8, IoError>) -> Result<u8, AppError> { r.map_err(AppError::from) }",
        pass: true,
    },
    Example {
        label: "closure wrapping only the error",
        code: "fn f(r: Result<u8, IoError>) -> Result<u8, AppError> { r.map_err(|e| AppError::Io(e)) }",
        pass: true,
    },
    Example {
        label: "closure adding context arguments",
        code: "fn f(r: Result<u8, IoError>) -> Result<u8, AppError> { r.map_err(|e| AppError::io(e, \"config.toml\")) }",
        pass: true,
    },
    Example {
        label: "closure building struct context",
        code: "fn f(r: Result<u8, IoError>) -> Result<u8, AppError> { r.map_err(|e| AppError { source: e }) }",
        pass: true,
    },
    Example {
        label: "closure transforming the error",
        code: "fn f(r: Result<u8, IoError>) -> Result<u8, AppError> { r.map_err(|e| AppError::parse(e.to_string())) }",
        pass: true,
    },
    Example {
        label: "closure discarding the error",
        code: "fn f(r: Result<u8, IoError>) -> Result<u8, AppError> { r.map_err(|_| AppError::Unknown) }",
        pass: true,
    },
    Example {
        label: "free fn argument",
        code: "fn f(r: Result<u8, IoError>) -> Result<u8, AppError> { r.map_err(convert) }",
        pass: true,
    },
    Example {
        label: "no map_err",
        code: "fn f(r: Result<u8, IoError>) -> Result<u8, IoError> { r }",
        pass: true,
    },
    Example {
        label: "pure wrap in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn f(r: Result<u8, IoError>) -> Result<u8, AppError> { r.map_err(AppError::Io) }\n}",
        pass: true,
    },
    Example {
        label: "trait implementation controls the error type",
        code: "struct Decoder;\nimpl Decode for Decoder { fn decode(r: Result<u8, SourceError>) -> Result<u8, D::Error> { r.map_err(D::Error::custom) } }",
        pass: true,
    },
];

crate::ast_rule!(
    map_err_pure_wrap,
    "Flag `.map_err(Type::from)` when `?` can already perform the same declared conversion.",
    "Repeating an existing From conversion obscures the happy path; use ? and let the declared conversion run automatically. Enum variants are not flagged because orphan rules or distinct semantic context may make a From impl invalid.",
    Low,
);

fn check_map_err_pure_wrap(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::MethodCallExpr>()
        .filter(|call| !ctx.is_in_test(call) && !is_in_trait_impl(call))
        .filter_map(|call| {
            let method = call.name_ref()?;

            if method.text() != "map_err" {
                return None;
            }

            let args = call.arg_list()?;
            let mut args = args.args();
            let arg = args.next()?;

            (args.next().is_none() && is_redundant_propagated_from(&call, &arg)).then(|| {
                ctx.violation(
                    &method,
                    ".map_err(Type::from) repeats a declared conversion — use `?`",
                )
            })
        })
        .collect()
}

fn is_redundant_propagated_from(call: &ast::MethodCallExpr, argument: &ast::Expr) -> bool {
    let immediately_propagated = call
        .syntax()
        .parent()
        .and_then(ast::TryExpr::cast)
        .is_some();
    let Some(target) = qualified_from_target(argument) else {
        return false;
    };

    immediately_propagated && enclosing_result_error(call).is_some_and(|error| error == target)
}

fn is_in_trait_impl(call: &ast::MethodCallExpr) -> bool {
    call.syntax()
        .ancestors()
        .find_map(ast::Impl::cast)
        .is_some_and(|item| item.trait_().is_some())
}

fn qualified_from_target(argument: &ast::Expr) -> Option<String> {
    let ast::Expr::PathExpr(_) = argument else {
        return None;
    };
    let compact = compact_type_syntax(argument.syntax());
    let qualified = compact.strip_prefix('<')?;
    let (target, conversion) = qualified.split_once("asFrom<")?;

    conversion.ends_with(">>::from").then(|| target.to_owned())
}

fn enclosing_result_error(call: &ast::MethodCallExpr) -> Option<String> {
    let function = call
        .syntax()
        .ancestors()
        .skip(1)
        .take_while(|ancestor| !ast::ClosureExpr::can_cast(ancestor.kind()))
        .find_map(ast::Fn::cast)?;
    let ast::Type::PathType(result) = function.ret_type()?.ty()? else {
        return None;
    };
    let segment = result.path()?.segment()?;

    if segment.name_ref()?.text() != "Result" {
        return None;
    }

    segment
        .generic_arg_list()?
        .generic_args()
        .nth(1)
        .and_then(|argument| match argument {
            ast::GenericArg::TypeArg(argument) => argument.ty(),
            _ => None,
        })
        .map(|ty| compact_type_syntax(ty.syntax()))
}

fn compact_type_syntax(node: &ra_ap_syntax::SyntaxNode) -> String {
    node.text()
        .to_string()
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

crate::rulewright_ast_test!(check_map_err_pure_wrap, {
    crate::example_tests!(EXAMPLES, check_map_err_pure_wrap);
});
