use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName},
    syntax_editor::SyntaxEditor,
};

use super::support::item_lists;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "sorted declarations",
        code: "mod alpha;\npub mod beta;\nmod gamma;",
        pass: true,
    },
    Example {
        label: "visibility does not affect ordering",
        code: "pub mod zebra;\nmod alpha;",
        pass: false,
    },
    Example {
        label: "separate declaration blocks",
        code: "mod zebra;\nfn boundary() {}\nmod alpha;",
        pass: true,
    },
    Example {
        label: "inline modules are boundaries",
        code: "mod zebra {}\nmod alpha;",
        pass: true,
    },
    Example {
        label: "raw identifiers sort by their semantic name",
        code: "mod parser;\nmod r#trait;\nmod writer;",
        pass: true,
    },
];

crate::ast_tree_rule!(
    mod_order,
    "Require contiguous module-declaration blocks to be alphabetically sorted.",
    "Stable module ordering makes module inventories predictable without grouping by visibility.",
    Low,
    fix_mod_order,
);

fn check_mod_order(ctx: &AstCtx<'_>) -> Vec<Violation> {
    item_lists(ctx.root)
        .flat_map(module_blocks)
        .filter_map(|block| {
            let names: Vec<String> = block.iter().filter_map(module_sort_key).collect();

            (!names.is_sorted()).then(|| {
                ctx.violation(
                    &block[0],
                    "module declarations are not alphabetically sorted",
                )
            })
        })
        .collect()
}

fn module_blocks(items: Vec<ast::Item>) -> impl Iterator<Item = Vec<ast::Module>> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();

    for item in items {
        if let ast::Item::Module(module) = item
            && module.item_list().is_none()
        {
            current.push(module);
        } else if !current.is_empty() {
            blocks.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        blocks.push(current);
    }

    blocks.into_iter().filter(|block| block.len() > 1)
}

fn fix_mod_order(ctx: &AstCtx<'_>, _violations: &[Violation]) -> Option<String> {
    let (editor, root) = SyntaxEditor::with_ast_node(ctx.root);
    let mut changed = false;

    for block in item_lists(&root).flat_map(module_blocks) {
        let mut target = block.clone();

        target.sort_by_key(module_sort_key);

        if block == target {
            continue;
        }

        for (old, new) in block.iter().zip(&target) {
            editor.replace(old.syntax().clone(), new.syntax().clone_subtree());
        }

        changed = true;
    }

    changed.then(|| editor.finish().new_root().to_string())
}

fn module_sort_key(module: &ast::Module) -> Option<String> {
    module
        .name()
        .map(|name| name.text().trim_start_matches("r#").to_owned())
}

crate::rulewright_ast_test!(check_mod_order, {
    crate::example_tests!(EXAMPLES, check_mod_order);
    crate::fix_tests!(EXAMPLES, ast_tree, check_mod_order, fix_mod_order);
});
