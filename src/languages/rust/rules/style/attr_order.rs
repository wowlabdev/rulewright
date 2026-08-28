use ra_ap_syntax::{
    AstNode, AstToken, SyntaxElement,
    ast::{self},
    syntax_editor::SyntaxEditor,
};

use crate::{AstCtx, Example, Violation};

const OTHER_ATTRIBUTE_RANK: u8 = 2;

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "docs derive then other attributes",
        code: "/// Item docs.\n#[derive(Debug)]\n#[cfg(test)]\nstruct Item;",
        pass: true,
    },
    Example {
        label: "derive follows other attribute",
        code: "#[cfg(test)]\n#[derive(Debug)]\nstruct Item;",
        pass: false,
    },
    Example {
        label: "docs must be first",
        code: "#[derive(Debug)]\n/// Item docs.\nstruct Item;",
        pass: false,
    },
    Example {
        label: "stable order within category",
        code: "#[cfg(unix)]\n#[allow(dead_code)]\nstruct Item;",
        pass: true,
    },
];

crate::ast_tree_rule!(
    attr_order,
    "Require item attributes to be ordered as docs, derives, then other attributes.",
    "Consistent attribute categories keep API documentation and generated traits prominent.",
    Low,
    fix_attr_order,
);

fn check_attr_order(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::Item>()
        .filter_map(|item| {
            let attributes = attribute_elements(&item);
            let ranks: Vec<u8> = attributes.iter().map(|(rank, _)| *rank).collect();

            (!ranks.is_sorted()).then(|| {
                ctx.violation(
                    &item,
                    "attributes must be ordered as documentation, derives, then other attributes",
                )
            })
        })
        .collect()
}

fn attribute_elements(item: &ast::Item) -> Vec<(u8, SyntaxElement)> {
    item.syntax()
        .children_with_tokens()
        .filter_map(|element| match &element {
            SyntaxElement::Token(token)
                if ast::Comment::cast(token.clone()).is_some_and(|comment| comment.is_doc()) =>
            {
                Some((0, element))
            }

            SyntaxElement::Node(node) => ast::Attr::cast(node.clone()).map(|attr| {
                let rank = if attr
                    .as_simple_call()
                    .is_some_and(|(name, _)| name == "derive")
                {
                    1
                } else {
                    OTHER_ATTRIBUTE_RANK
                };

                (rank, element)
            }),

            SyntaxElement::Token(_) => None,
        })
        .collect()
}

fn fix_attr_order(ctx: &AstCtx<'_>, _violations: &[Violation]) -> Option<String> {
    let (editor, root) = SyntaxEditor::with_ast_node(ctx.root);
    let mut changed = false;

    for item in root.syntax().descendants().filter_map(ast::Item::cast) {
        let attributes = attribute_elements(&item);
        let mut target = attributes.clone();

        target.sort_by_key(|(rank, _)| *rank);

        if attributes == target {
            continue;
        }

        for ((_, old), (_, new)) in attributes.iter().zip(&target) {
            editor.replace(old.clone(), new.clone());
        }

        changed = true;
    }

    changed.then(|| editor.finish().new_root().to_string())
}

crate::rulewright_ast_test!(check_attr_order, {
    crate::example_tests!(EXAMPLES, check_attr_order);
    crate::fix_tests!(EXAMPLES, ast_tree, check_attr_order, fix_attr_order);
});
