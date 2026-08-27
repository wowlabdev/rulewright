use ra_ap_syntax::{AstNode, ast, ast::HasArgList};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "bare variant path",
        code: "fn f(r: Result<u8, IoError>) -> Result<u8, AppError> { r.map_err(AppError::Io) }",
        pass: false,
    },
    Example {
        label: "bare from path",
        code: "fn f(r: Result<u8, IoError>) -> Result<u8, AppError> { r.map_err(AppError::from) }",
        pass: false,
    },
    Example {
        label: "closure wrapping only the error",
        code: "fn f(r: Result<u8, IoError>) -> Result<u8, AppError> { r.map_err(|e| AppError::Io(e)) }",
        pass: false,
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
];

crate::ast_rule!(
    map_err_pure_wrap,
    "Flag `.map_err(...)` that only wraps the error in another type — implement `From` and let `?` convert.",
    "Context-free error wrapping repeated at every call site obscures the happy path; a single From impl gives the conversion to every ? for free.",
    Low,
);

fn check_map_err_pure_wrap(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::MethodCallExpr>()
        .filter(|call| !ctx.is_in_test(call))
        .filter_map(|call| {
            let method = call.name_ref()?;

            if method.text() != "map_err" {
                return None;
            }

            let args = call.arg_list()?;
            let mut args = args.args();
            let arg = args.next()?;

            (args.next().is_none() && is_pure_wrap(&arg)).then(|| {
                ctx.violation(
                    &method,
                    ".map_err() wraps the error without adding context — implement `From` and use `?`",
                )
            })
        })
        .collect()
}

fn is_pure_wrap(arg: &ast::Expr) -> bool {
    match arg {
        ast::Expr::PathExpr(path) => path.path().is_some_and(|path| is_ctor_like_path(&path)),
        ast::Expr::ClosureExpr(closure) => closure_pure_wraps(closure),
        _ => false,
    }
}

fn is_ctor_like_path(path: &ast::Path) -> bool {
    if path.qualifier().is_some() {
        return true;
    }

    path.segment()
        .and_then(|seg| seg.name_ref())
        .is_some_and(|name| name.text().chars().next().is_some_and(char::is_uppercase))
}

fn closure_pure_wraps(closure: &ast::ClosureExpr) -> bool {
    let normalized: String = closure
        .syntax()
        .text()
        .to_string()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect();
    let Some(rest) = normalized.strip_prefix('|') else {
        return false;
    };
    let Some((binding, body)) = rest.split_once('|') else {
        return false;
    };

    if binding.is_empty() || binding == "_" || binding.contains(',') {
        return false;
    }

    let body = body
        .strip_prefix('{')
        .and_then(|body| body.strip_suffix('}'))
        .unwrap_or(body);
    let Some(open) = body.find('(') else {
        return false;
    };
    let Some(argument) = body.get(open + 1..body.len().saturating_sub(1)) else {
        return false;
    };

    body.ends_with(')')
        && argument == binding
        && body
            .get(..open)
            .is_some_and(|constructor| !constructor.is_empty())
}

crate::rulewright_ast_test!(check_map_err_pure_wrap, {
    crate::example_tests!(EXAMPLES, check_map_err_pure_wrap);
});
