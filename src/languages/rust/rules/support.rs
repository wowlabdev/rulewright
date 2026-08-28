use ra_ap_syntax::{
    AstNode, Edition, SourceFile, SyntaxElement, SyntaxNode,
    ast::{self, HasModuleItem, LiteralKind},
};

pub(super) fn compact_syntax(node: &SyntaxNode) -> String {
    node.descendants_with_tokens()
        .filter_map(ra_ap_syntax::NodeOrToken::into_token)
        .filter(|token| !token.kind().is_trivia())
        .fold(String::new(), |mut source, token| {
            source.push_str(token.text());

            source
        })
}

pub(super) fn is_in_const_or_static<N>(node: &N) -> bool
where
    N: AstNode,
{
    node.syntax().ancestors().any(|ancestor| {
        ast::Const::can_cast(ancestor.kind()) || ast::Static::can_cast(ancestor.kind())
    })
}

pub(super) fn macro_arguments(call: &ast::MacroCall) -> Option<Vec<String>> {
    let tree = call.token_tree()?;
    let mut elements: Vec<SyntaxElement> = tree.syntax().children_with_tokens().collect();

    elements.first()?.as_token()?;
    elements.last()?.as_token()?;
    elements.remove(0);
    elements.pop();
    let mut arguments = vec![String::new()];

    for element in elements {
        if element.as_token().is_some_and(|token| token.text() == ",") {
            arguments.push(String::new());
        } else if let Some(argument) = arguments.last_mut() {
            argument.push_str(&element.to_string());
        }
    }

    Some(arguments)
}

const ONE_BYTE: u64 = 1;
const TWO_BYTES: u64 = 2;
const FOUR_BYTES: u64 = 4;
const EIGHT_BYTES: u64 = 8;
const SIXTEEN_BYTES: u64 = 16;
const THREE_WORDS: u64 = 24;

pub(super) fn parse_use(source: &str) -> Option<ast::Use> {
    let parse = SourceFile::parse(source, Edition::Edition2024);

    parse
        .errors()
        .is_empty()
        .then(|| parse.tree())?
        .items()
        .find_map(|item| match item {
            ast::Item::Use(item) => Some(item),
            _ => None,
        })
}

pub(super) fn type_name(ty: &ast::Type) -> Option<String> {
    let ast::Type::PathType(path_type) = ty else {
        return None;
    };

    path_type
        .path()?
        .segment()?
        .name_ref()
        .map(|name| name.text().to_string())
}

pub(super) fn is_inside_trait(function: &ast::Fn) -> bool {
    function
        .syntax()
        .ancestors()
        .skip(1)
        .take_while(|node| ast::Fn::cast(node.clone()).is_none())
        .any(|node| ast::Trait::cast(node).is_some())
}

pub(super) fn is_item_or_impl_fn(function: &ast::Fn) -> bool {
    let Some(parent) = function.syntax().parent() else {
        return false;
    };

    if ast::ExternItemList::cast(parent.clone()).is_some() {
        return false;
    }

    let Some(items) = ast::AssocItemList::cast(parent) else {
        return true;
    };

    items.syntax().parent().and_then(ast::Impl::cast).is_some()
}

pub(super) fn path_names(path: &ast::Path) -> Vec<String> {
    let mut names = Vec::new();
    let mut current = Some(path.clone());

    while let Some(path) = current {
        if let Some(name) = path.segment().and_then(|segment| segment.name_ref()) {
            names.push(name.text().to_string());
        }

        current = path.qualifier();
    }

    names.reverse();

    names
}

// #rw(fn: rust_recursive_fn) composite types form a shallow syntax tree and need recursive size aggregation
pub(super) fn estimate_type_size(ty: &ast::Type) -> Option<u64> {
    match ty {
        ast::Type::PathType(path) => estimate_path_size(path),

        ast::Type::ArrayType(array) => {
            let element = estimate_type_size(&array.ty()?)?;
            let length = parse_int_expr(&array.const_arg()?.expr()?)?;

            Some(element.saturating_mul(length))
        }

        ast::Type::TupleType(tuple) => tuple.fields().try_fold(0u64, |total, element| {
            Some(total.saturating_add(estimate_type_size(&element)?))
        }),

        ast::Type::RefType(_) | ast::Type::PtrType(_) => Some(EIGHT_BYTES),

        _ => None,
    }
}

fn estimate_path_size(path: &ast::PathType) -> Option<u64> {
    let name = path.path()?.segment()?.name_ref()?;

    match name.text().as_str() {
        "bool" | "u8" | "i8" => Some(ONE_BYTE),
        "u16" | "i16" => Some(TWO_BYTES),
        "u32" | "i32" | "f32" => Some(FOUR_BYTES),
        "u64" | "i64" | "f64" | "usize" | "isize" | "Box" | "Arc" | "Rc" => Some(EIGHT_BYTES),
        "u128" | "i128" => Some(SIXTEEN_BYTES),
        "Vec" | "String" => Some(THREE_WORDS),
        _ => None,
    }
}

pub(super) fn parse_int_expr(expr: &ast::Expr) -> Option<u64> {
    let ast::Expr::Literal(literal) = expr else {
        return None;
    };
    let LiteralKind::IntNumber(number) = literal.kind() else {
        return None;
    };

    number.value().ok()?.try_into().ok()
}
