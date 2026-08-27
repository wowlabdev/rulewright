use ra_ap_syntax::{
    AstNode,
    ast::{self, HasVisibility},
    syntax_editor::SyntaxEditor,
};
use std::collections::{HashMap, HashSet};

use super::support::item_lists;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "origins are adjacent",
        code: "pub use alpha::One;\npub use alpha::Two;\npub use beta::Three;",
        pass: true,
    },
    Example {
        label: "origin repeats after another group",
        code: "pub use alpha::One;\npub use beta::Two;\npub use alpha::Three;",
        pass: false,
    },
    Example {
        label: "origin group order is author chosen",
        code: "pub use zebra::One;\npub use alpha::Two;",
        pass: true,
    },
    Example {
        label: "plain use separates blocks",
        code: "pub use alpha::One;\nuse beta::Two;\npub use alpha::Three;",
        pass: true,
    },
];

crate::ast_tree_rule!(
    pub_use_grouping,
    "Require public re-exports from the same origin to be adjacent.",
    "Keeping each re-export origin contiguous makes public API inventories easier to scan.",
    Low,
    fix_pub_use_grouping,
);

fn check_pub_use_grouping(ctx: &AstCtx<'_>) -> Vec<Violation> {
    item_lists(ctx.root)
        .flat_map(public_use_blocks)
        .filter_map(|block| {
            (!origins_are_grouped(&block)).then(|| {
                ctx.violation(
                    &block[0],
                    "public re-exports from the same first path segment must be adjacent",
                )
            })
        })
        .collect()
}

fn public_use_blocks(items: Vec<ast::Item>) -> impl Iterator<Item = Vec<ast::Use>> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();

    for item in items {
        if let ast::Item::Use(use_item) = item
            && use_item.visibility().is_some()
        {
            current.push(use_item);
        } else if !current.is_empty() {
            blocks.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        blocks.push(current);
    }

    blocks.into_iter().filter(|block| block.len() > 1)
}

fn origin(item: &ast::Use) -> String {
    let Some(mut path) = item.use_tree().and_then(|tree| tree.path()) else {
        return String::new();
    };

    while let Some(qualifier) = path.qualifier() {
        path = qualifier;
    }

    path.segment()
        .and_then(|segment| segment.name_ref())
        .map_or_else(String::new, |name| name.text().to_string())
}

fn origins_are_grouped(block: &[ast::Use]) -> bool {
    let mut closed = HashSet::<String>::new();
    let mut previous = None;

    for current in block.iter().map(origin) {
        if previous.as_ref().is_some_and(|last| last != &current) {
            closed.insert(previous.take().unwrap_or_default());
        }

        if closed.contains(&current) {
            return false;
        }

        previous = Some(current);
    }

    true
}

// #rw(fn: rust_clone_in_loop) syntax-editor sorting requires detached copies of source items
fn fix_pub_use_grouping(ctx: &AstCtx<'_>, _violations: &[Violation]) -> Option<String> {
    let (editor, root) = SyntaxEditor::with_ast_node(ctx.root);
    let mut changed = false;

    for block in item_lists(&root).flat_map(public_use_blocks) {
        if origins_are_grouped(&block) {
            continue;
        }

        let mut groups: Vec<Vec<ast::Use>> = Vec::with_capacity(block.len());
        let mut positions = HashMap::<String, usize>::default();

        for item in &block {
            let key = origin(item);
            let next = positions.len();
            let index = *positions.entry(key).or_insert_with(|| {
                groups.push(Vec::new());

                next
            });

            groups
                .get_mut(index)
                .expect("origin position always names an initialized group")
                .push(item.clone());
        }

        let target: Vec<ast::Use> = groups.into_iter().flatten().collect();

        for (old, new) in block.iter().zip(&target) {
            editor.replace(old.syntax().clone(), new.syntax().clone_subtree());
        }

        changed = true;
    }

    changed.then(|| editor.finish().new_root().to_string())
}

crate::rulewright_ast_test!(check_pub_use_grouping, {
    crate::example_tests!(EXAMPLES, check_pub_use_grouping);
    crate::fix_tests!(ast_tree, check_pub_use_grouping, fix_pub_use_grouping);
});
