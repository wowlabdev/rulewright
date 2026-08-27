use ra_ap_syntax::{
    AstNode,
    ast::{self, HasModuleItem},
};

pub(super) fn item_lists(root: &ast::SourceFile) -> impl Iterator<Item = Vec<ast::Item>> + '_ {
    std::iter::once(root.items().collect()).chain(
        root.syntax()
            .descendants()
            .filter_map(ast::ItemList::cast)
            .map(|list| list.items().collect()),
    )
}
