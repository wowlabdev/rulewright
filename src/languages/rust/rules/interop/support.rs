use ra_ap_syntax::{
    AstNode,
    ast::{self, HasAttrs, HasName, HasVisibility, VisibilityKind},
};

pub(super) use super::super::support::{is_inside_trait, is_item_or_impl_fn};

pub(super) fn is_fully_public(node: &impl HasVisibility) -> bool {
    node.visibility()
        .is_some_and(|visibility| matches!(visibility.kind(), VisibilityKind::Pub))
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "ra_ap_syntax traversal callbacks yield owned paths and this helper is a callback target"
)]
pub(super) fn path_names(path: ast::Path) -> Vec<String> {
    super::super::support::path_names(&path)
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "ra_ap_syntax traversal callbacks yield owned paths for last-segment checks"
)]
pub(super) fn path_last_is(path: ast::Path, expected: &str) -> bool {
    path.segment()
        .and_then(|segment| segment.name_ref())
        .is_some_and(|name| name.text() == expected)
}

pub(super) fn path_matches(path: ast::Path, expected: &[&str]) -> bool {
    path_names(path)
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "ra_ap_syntax traversal callbacks yield owned types for name extraction"
)]
pub(super) fn type_name(ty: ast::Type) -> Option<String> {
    super::super::support::type_name(&ty)
}

pub(super) fn has_cfg_feature(node: &impl HasAttrs) -> bool {
    node.attrs().any(|attr| {
        attr.simple_name().is_some_and(|name| name == "cfg")
            && attr
                .syntax()
                .descendants_with_tokens()
                .filter_map(ra_ap_syntax::NodeOrToken::into_token)
                .any(|token| token.text() == "feature")
    })
}

pub(super) fn field_types(list: ast::FieldList) -> Vec<ast::Type> {
    match list {
        ast::FieldList::RecordFieldList(fields) => {
            fields.fields().filter_map(|field| field.ty()).collect()
        }
        ast::FieldList::TupleFieldList(fields) => {
            fields.fields().filter_map(|field| field.ty()).collect()
        }
    }
}

pub(super) fn enclosing_impl(function: &ast::Fn) -> Option<ast::Impl> {
    function
        .syntax()
        .ancestors()
        .skip(1)
        .find_map(ast::Impl::cast)
}

pub(super) fn name_text(node: &impl HasName) -> Option<String> {
    node.name().map(|name| name.text().to_string())
}
