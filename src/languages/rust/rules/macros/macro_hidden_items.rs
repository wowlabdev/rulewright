use line_index::LineIndex;
use ra_ap_syntax::{
    AstNode, Edition, SourceFile, SyntaxKind,
    ast::{self, HasName, HasVisibility, VisibilityKind},
};

use super::support::is_quote_call;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "fixed-name pub struct in quote body",
        code: "fn expand() {\n    let _ = quote::quote! { pub struct Generated; };\n}",
        pass: false,
    },
    Example {
        label: "fixed-name pub fn in quote body",
        code: "fn expand() {\n    let _ = quote::quote! { pub fn helper() {} };\n}",
        pass: false,
    },
    Example {
        label: "fixed-name pub enum in quote body",
        code: "fn expand() {\n    let _ = quote::quote! { pub enum Kind {} };\n}",
        pass: false,
    },
    Example {
        label: "fixed-name pub trait in quote body",
        code: "fn expand() {\n    let _ = quote::quote! { pub trait Ext {} };\n}",
        pass: false,
    },
    Example {
        label: "interpolated ident passes",
        code: "fn expand() {\n    let _ = quote::quote! { pub struct #name; };\n}",
        pass: true,
    },
    Example {
        label: "repeated interpolation passes",
        code: "fn expand() {\n    let _ = quote::quote! { #(pub fn #getters();)* };\n}",
        pass: true,
    },
    Example {
        label: "private emitted item passes",
        code: "fn expand() {\n    let _ = quote::quote! { struct Hidden; };\n}",
        pass: true,
    },
    Example {
        label: "pub item outside quote body",
        code: "pub struct Normal;",
        pass: true,
    },
    Example {
        label: "opener in comment",
        code: "// quote! { pub struct Generated; }",
        pass: true,
    },
];

crate::ast_rule!(
    macro_hidden_items,
    "Flag fixed-name `pub` items emitted from quote! bodies.",
    "Fixed-name emitted items collide across expansions and clash with user code — interpolate the identifier instead.",
    Medium,
);

fn check_macro_hidden_items(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let quote_trees = ctx
        .nodes::<ast::MacroCall>()
        .filter(|call| !ctx.is_in_test(call) && is_quote_call(call))
        .filter_map(|call| call.token_tree());

    quote_trees
        .flat_map(|tree| emitted_public_items(ctx, &tree))
        .collect()
}

fn emitted_public_items(ctx: &AstCtx<'_>, tree: &ast::TokenTree) -> Vec<Violation> {
    let Some((source, body_line)) = token_tree_body(ctx, tree) else {
        return Vec::new();
    };
    let parse = SourceFile::parse(&source, Edition::Edition2024);
    let root = parse.tree();
    let line_index = LineIndex::new(&source);

    root.syntax()
        .descendants()
        .filter_map(public_item)
        .map(|(item, ident)| {
            let relative_line = line_index.line_col(item.text_range().start()).line as usize;

            crate::violation(
                ctx.file.rel,
                body_line + relative_line,
                format!(
                    "macro emits fixed-name public item `{ident}` — fixed names collide \
                     across expansions, interpolate the identifier"
                ),
            )
        })
        .collect()
}

fn public_item(node: ra_ap_syntax::SyntaxNode) -> Option<(ra_ap_syntax::SyntaxNode, String)> {
    if !matches!(
        node.kind(),
        SyntaxKind::STRUCT | SyntaxKind::ENUM | SyntaxKind::TRAIT | SyntaxKind::FN
    ) {
        return None;
    }

    let visibility = ast::AnyHasVisibility::cast(node.clone())?.visibility()?;

    if !matches!(visibility.kind(), VisibilityKind::Pub) {
        return None;
    }

    let name = ast::AnyHasName::cast(node.clone())?
        .name()?
        .text()
        .to_string();

    Some((node, name))
}

fn token_tree_body(ctx: &AstCtx<'_>, tree: &ast::TokenTree) -> Option<(String, usize)> {
    let opener = tree
        .l_paren_token()
        .or_else(|| tree.l_brack_token())
        .or_else(|| tree.l_curly_token())?;
    let closer = tree
        .r_paren_token()
        .or_else(|| tree.r_brack_token())
        .or_else(|| tree.r_curly_token())?;
    let start = u32::from(opener.text_range().end()) as usize;
    let end = u32::from(closer.text_range().start()) as usize;
    let source = ctx.file.contents.get(start..end).map(str::to_owned)?;
    let body_line = ctx.line_index.line_col(opener.text_range().start()).line as usize + 1;

    Some((source, body_line))
}

crate::rulewright_ast_test!(check_macro_hidden_items, {
    crate::example_tests!(EXAMPLES, check_macro_hidden_items);
});
