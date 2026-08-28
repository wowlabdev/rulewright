use ra_ap_syntax::{
    AstNode, SyntaxKind, SyntaxNode, SyntaxToken,
    ast::{self, HasModuleItem},
};

const BLANK_LINE_NEWLINES: usize = 2;

pub(super) fn item_lists(root: &ast::SourceFile) -> impl Iterator<Item = Vec<ast::Item>> + '_ {
    std::iter::once(root.items().collect()).chain(
        root.syntax()
            .descendants()
            .filter_map(ast::ItemList::cast)
            .map(|list| list.items().collect()),
    )
}

pub(super) fn gap_before_attached_comment(current: &SyntaxNode) -> Option<SyntaxToken> {
    let mut gap = direct_gap(current)?;

    if !gap.text().contains('\n')
        && let Some(comment) = current
            .first_token()
            .filter(|token| token.kind() == SyntaxKind::COMMENT)
        && let Some(after_comment) = comment
            .next_token()
            .filter(|token| token.kind() == SyntaxKind::WHITESPACE)
    {
        return Some(after_comment);
    }

    while !has_blank_line(&gap) {
        let Some(comment) = gap
            .prev_token()
            .filter(|token| token.kind() == SyntaxKind::COMMENT && starts_own_line(token))
        else {
            break;
        };
        let Some(previous_gap) = comment
            .prev_token()
            .filter(|token| token.kind() == SyntaxKind::WHITESPACE)
        else {
            break;
        };

        gap = previous_gap;
    }

    Some(gap)
}

pub(super) fn has_blank_line(token: &SyntaxToken) -> bool {
    token.text().matches('\n').count() >= BLANK_LINE_NEWLINES
}

fn direct_gap(current: &SyntaxNode) -> Option<SyntaxToken> {
    current
        .prev_sibling_or_token()
        .and_then(ra_ap_syntax::NodeOrToken::into_token)
        .filter(|token| token.kind() == SyntaxKind::WHITESPACE)
}

fn starts_own_line(comment: &SyntaxToken) -> bool {
    comment.prev_token().is_some_and(|previous| {
        previous.kind() == SyntaxKind::WHITESPACE && previous.text().contains('\n')
    })
}
