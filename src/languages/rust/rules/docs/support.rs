use ra_ap_syntax::{
    AstNode,
    ast::{self, HasAttrs, HasDocComments, LiteralKind},
};

pub(super) fn is_item_or_impl_fn(function: &ast::Fn) -> bool {
    !function.syntax().ancestors().skip(1).any(|ancestor| {
        ast::Trait::can_cast(ancestor.kind()) || ast::ExternBlock::can_cast(ancestor.kind())
    })
}

pub(super) fn doc_lines(function: &ast::Fn) -> Vec<String> {
    let mut lines: Vec<String> = function
        .doc_comments()
        .filter_map(|comment| comment.doc_comment().map(|(text, _)| text.to_owned()))
        .collect();

    lines.extend(function.attrs().filter_map(|attr| doc_attr_text(&attr)));

    lines
}

fn doc_attr_text(attr: &ast::Attr) -> Option<String> {
    let ast::Meta::KeyValueMeta(meta) = attr.meta()? else {
        return None;
    };

    let name = meta
        .path()
        .and_then(|path| path.segment())
        .and_then(|segment| segment.name_ref());

    if name.is_none_or(|name| name.text() != "doc") {
        return None;
    }

    let ast::Expr::Literal(literal) = meta.expr()? else {
        return None;
    };
    let LiteralKind::String(text) = literal.kind() else {
        return None;
    };

    text.value().ok().map(std::borrow::Cow::into_owned)
}

pub(super) fn has_heading(doc_lines: &[String], name: &str) -> bool {
    doc_lines
        .iter()
        .flat_map(|chunk| chunk.lines())
        .any(|line| {
            let trimmed = line.trim();
            let stripped = trimmed.trim_start_matches('#');

            stripped.len() < trimmed.len() && stripped.trim() == name
        })
}
