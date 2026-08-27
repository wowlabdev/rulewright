use ra_ap_syntax::{
    AstNode,
    ast::{self, HasAttrs},
};

// #rw(rust_similar_fns) Generic derive lookup intentionally differs from the error rule's specialized Error-derive classification.
pub(super) fn has_derive(item: &impl HasAttrs, target: &str) -> bool {
    item.attrs().any(|attr| {
        attr.as_simple_call().is_some_and(|(name, tokens)| {
            name == "derive"
                && tokens.syntax().text().to_string().split(',').any(|entry| {
                    entry
                        .trim_matches(|ch: char| ch.is_whitespace() || matches!(ch, '(' | ')'))
                        .rsplit("::")
                        .next()
                        == Some(target)
                })
        })
    })
}

pub(super) fn type_name(ty: &ast::Type) -> Option<String> {
    let ast::Type::PathType(path) = ty else {
        return None;
    };

    path.path()?
        .segment()?
        .name_ref()
        .map(|name| name.text().to_string())
}

pub(super) fn path_reachable_from(root: &ast::Type, path: &ast::PathType) -> bool {
    path.syntax()
        .ancestors()
        .take_while(|node| node != root.syntax())
        .filter_map(ast::Type::cast)
        .all(|ty| {
            matches!(
                ty,
                ast::Type::PathType(_)
                    | ast::Type::RefType(_)
                    | ast::Type::SliceType(_)
                    | ast::Type::ArrayType(_)
                    | ast::Type::ParenType(_)
                    | ast::Type::TupleType(_)
            )
        })
        && matches!(
            root,
            ast::Type::PathType(_)
                | ast::Type::RefType(_)
                | ast::Type::SliceType(_)
                | ast::Type::ArrayType(_)
                | ast::Type::ParenType(_)
                | ast::Type::TupleType(_)
        )
}
