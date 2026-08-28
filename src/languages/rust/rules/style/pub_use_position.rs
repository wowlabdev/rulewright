#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{
    AstNode, SourceFile,
    ast::{self, HasAttrs, HasModuleItem, HasVisibility},
    syntax_editor::SyntaxEditor,
};

use super::super::support::parse_use;
use crate::{AstCtx, Example, Violation};

const BLANK_LINE_NEWLINES: usize = 2;

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "plain imports before separated public imports",
        code: "use std::fmt;\nuse std::io;\n\npub use crate::api::Thing;\npub use crate::api::Other;",
        pass: true,
    },
    Example {
        label: "public import before plain import",
        code: "pub use crate::api::Thing;\nuse std::fmt;",
        pass: false,
    },
    Example {
        label: "blocks need a blank line",
        code: "use std::fmt;\npub use crate::api::Thing;",
        pass: false,
    },
    Example {
        label: "inline module is exempt",
        code: "mod inner {\n    pub use crate::api::Thing;\n    use std::fmt;\n}",
        pass: true,
    },
    Example {
        label: "only public imports",
        code: "pub use crate::api::Thing;\npub use crate::api::Other;",
        pass: true,
    },
];

crate::ast_tree_rule!(
    pub_use_position,
    "Require top-level public imports to follow plain imports in a separate block.",
    "Keeping imports and re-exports in distinct leading blocks makes module API boundaries visible.",
    Low,
    fix_pub_use_position,
);

fn check_pub_use_position(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let uses = leading_uses(ctx.root);
    let plain_count = uses.iter().filter(|item| !is_public(item)).count();

    if plain_count == 0 || plain_count == uses.len() {
        return Vec::new();
    }

    let ordered = uses
        .iter()
        .enumerate()
        .all(|(index, item)| is_public(item) == (index >= plain_count));
    let (plain, public) = uses.split_at(plain_count);
    let separated = ordered
        && plain
            .last()
            .zip(public.first())
            .is_some_and(|(before, after)| has_blank_line(ctx, before, after));

    if ordered && separated {
        return Vec::new();
    }

    uses.get(plain_count.min(uses.len().saturating_sub(1)))
        .map(|item| {
            vec![ctx.violation(
                item,
                "public imports must follow all plain imports in a blank-line-separated block",
            )]
        })
        .unwrap_or_default()
}

fn leading_uses(root: &SourceFile) -> Vec<ast::Use> {
    let items: Vec<ast::Item> = root.items().collect();
    let Some(start) = items
        .iter()
        .position(|item| matches!(item, ast::Item::Use(_)))
    else {
        return Vec::new();
    };

    items
        .get(start..)
        .unwrap_or_default()
        .iter()
        .map_while(|item| match item {
            ast::Item::Use(item) => Some(item.clone()),
            _ => None,
        })
        .collect()
}

fn is_public(item: &ast::Use) -> bool {
    item.visibility().is_some()
}

fn has_blank_line(ctx: &AstCtx<'_>, before: &ast::Use, after: &ast::Use) -> bool {
    let start: usize = before.syntax().text_range().end().into();
    let end: usize = after.syntax().text_range().start().into();

    ctx.file
        .contents
        .get(start..end)
        .is_some_and(|between| between.matches('\n').count() >= BLANK_LINE_NEWLINES)
}

fn fix_pub_use_position(ctx: &AstCtx<'_>, _violations: &[Violation]) -> Option<String> {
    let (editor, root) = SyntaxEditor::with_ast_node(ctx.root);
    let uses = leading_uses(&root);
    let plain_count = uses.iter().filter(|item| !is_public(item)).count();

    if plain_count == 0 || plain_count == uses.len() {
        return None;
    }

    let mut target = uses.clone();

    target.sort_by_key(is_public);

    for item in &mut target {
        if is_public(item) && !has_rustfmt_skip(item) {
            *item = parse_use(&format!("#[rustfmt::skip]\n{}", item.syntax()))?;
        }
    }

    for (old, new) in uses.iter().zip(&target) {
        if old != new {
            editor.replace(old.syntax().clone(), new.syntax().clone_subtree());
        }
    }

    if let Some(whitespace) = uses
        .get(plain_count.saturating_sub(1))?
        .syntax()
        .next_sibling_or_token()
        .and_then(ra_ap_syntax::NodeOrToken::into_token)
        .filter(|token| token.kind().is_trivia())
    {
        editor.replace(whitespace, editor.make().whitespace("\n\n"));
    }

    Some(editor.finish().new_root().to_string())
}

fn has_rustfmt_skip(item: &ast::Use) -> bool {
    item.attrs().any(|attr| {
        attr.syntax()
            .text()
            .to_string()
            .split_whitespace()
            .collect::<String>()
            == "#[rustfmt::skip]"
    })
}

crate::rulewright_ast_test!(check_pub_use_position, {
    crate::example_tests!(EXAMPLES, check_pub_use_position);
    crate::fix_tests!(
        EXAMPLES,
        ast_tree,
        check_pub_use_position,
        fix_pub_use_position
    );

    #[gtest]
    fn fix_marks_public_uses_to_keep_nightly_rustfmt_from_regrouping_them() -> Result<()> {
        let source = "pub use crate::api::Thing;\nuse std::fmt;";
        let fixed = crate::apply_ast_tree_fix(source, check_pub_use_position, fix_pub_use_position);

        verify_true!(fixed.contains("#[rustfmt::skip]\npub use crate::api::Thing;"))?;

        Ok(())
    }
});
