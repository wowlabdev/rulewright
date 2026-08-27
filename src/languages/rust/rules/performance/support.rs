use ra_ap_syntax::{
    AstNode, SyntaxNode,
    ast::{self, HasArgList, HasGenericArgs, HasLoopBody, HasName, LiteralKind},
};

pub(super) use super::super::support::{estimate_type_size, parse_int_expr};

pub(super) fn type_arg_last_ident(segment: &ast::PathSegment) -> Option<String> {
    segment.generic_arg_list()?.generic_args().find_map(|arg| {
        let ast::GenericArg::TypeArg(arg) = arg else {
            return None;
        };
        let ast::Type::PathType(path_type) = arg.ty()? else {
            return None;
        };

        path_type
            .path()?
            .segment()?
            .name_ref()
            .map(|name| name.text().to_string())
    })
}

const ONE_BYTE: u64 = 1;
const TWO_BYTES: u64 = 2;
const FOUR_BYTES: u64 = 4;
const EIGHT_BYTES: u64 = 8;
const SIXTEEN_BYTES: u64 = 16;

pub(super) fn loop_body<N>(node: &N) -> Option<ast::BlockExpr>
where
    N: AstNode,
{
    let syntax = node.syntax().clone();

    if let Some(loop_expr) = ast::ForExpr::cast(syntax.clone()) {
        return loop_expr.loop_body();
    }

    if let Some(loop_expr) = ast::WhileExpr::cast(syntax.clone()) {
        return loop_expr.loop_body();
    }

    ast::LoopExpr::cast(syntax).and_then(|loop_expr| loop_expr.loop_body())
}

pub(super) fn loop_source<N>(node: &N) -> Option<ast::Expr>
where
    N: AstNode,
{
    let syntax = node.syntax().clone();

    if let Some(loop_expr) = ast::ForExpr::cast(syntax.clone()) {
        return loop_expr.iterable();
    }

    ast::WhileExpr::cast(syntax).and_then(|loop_expr| loop_expr.condition())
}

pub(super) fn is_inside_loop_body<N>(node: &N) -> bool
where
    N: AstNode,
{
    node.syntax().ancestors().any(|ancestor| {
        ast::BlockExpr::cast(ancestor).is_some_and(|block| {
            block.syntax().parent().is_some_and(|parent| {
                ast::ForExpr::can_cast(parent.kind())
                    || ast::WhileExpr::can_cast(parent.kind())
                    || ast::LoopExpr::can_cast(parent.kind())
            })
        })
    })
}

pub(super) fn is_in_async_context<N>(node: &N) -> bool
where
    N: AstNode,
{
    for ancestor in node.syntax().ancestors().skip(1) {
        // #rw(rust_clone_in_loop) SyntaxNode clone is reference-counted and O(1)
        if let Some(closure) = ast::ClosureExpr::cast(ancestor.clone()) {
            return closure.async_token().is_some();
        }

        // #rw(rust_clone_in_loop) SyntaxNode clone is reference-counted and O(1)
        if let Some(block) = ast::BlockExpr::cast(ancestor.clone()) {
            if block.async_token().is_some() {
                return true;
            }

            continue;
        }

        if let Some(function) = ast::Fn::cast(ancestor) {
            return function.async_token().is_some();
        }
    }

    false
}

pub(super) use super::super::support::path_names;

pub(super) fn path_expr_name(expr: &ast::Expr) -> Option<String> {
    let ast::Expr::PathExpr(path_expr) = expr else {
        return None;
    };
    let path = path_expr.path()?;

    (path.qualifier().is_none()).then(|| {
        path.segment()?
            .name_ref()
            .map(|name| name.text().to_string())
    })?
}

pub(super) fn path_expr_last_name(expr: &ast::Expr) -> Option<String> {
    let ast::Expr::PathExpr(path_expr) = expr else {
        return None;
    };

    path_expr
        .path()?
        .segment()?
        .name_ref()
        .map(|name| name.text().to_string())
}

pub(super) fn method_name(call: &ast::MethodCallExpr) -> Option<String> {
    call.name_ref().map(|name| name.text().to_string())
}

pub(super) fn has_no_args<N>(call: &N) -> bool
where
    N: HasArgList,
{
    call.arg_list()
        .is_none_or(|arguments| arguments.args().next().is_none())
}

pub(super) fn ident_pattern_name(pattern: ast::Pat) -> Option<String> {
    let ast::Pat::IdentPat(pattern) = pattern else {
        return None;
    };

    pattern.name().map(|name| name.text().to_string())
}

pub(super) fn syntax_contains_ident(syntax: &SyntaxNode, name: &str) -> bool {
    syntax
        .descendants()
        .filter_map(ast::PathExpr::cast)
        .any(|path_expr| {
            path_expr.path().is_some_and(|path| {
                path.qualifier().is_none()
                    && path
                        .segment()
                        .and_then(|segment| segment.name_ref())
                        .is_some_and(|ident| ident.text() == name)
            })
        })
}

pub(super) fn literal_elem_size(expr: &ast::Expr) -> Option<u64> {
    let ast::Expr::Literal(literal) = expr else {
        return None;
    };
    let suffix = match literal.kind() {
        LiteralKind::IntNumber(number) => number.suffix().map(str::to_owned),
        LiteralKind::FloatNumber(number) => number.suffix().map(str::to_owned),
        _ => None,
    }?;

    match suffix.as_str() {
        "u8" | "i8" => Some(ONE_BYTE),
        "u16" | "i16" => Some(TWO_BYTES),
        "u32" | "i32" | "f32" => Some(FOUR_BYTES),
        "u64" | "i64" | "usize" | "isize" | "f64" => Some(EIGHT_BYTES),
        "u128" | "i128" => Some(SIXTEEN_BYTES),
        _ => None,
    }
}
