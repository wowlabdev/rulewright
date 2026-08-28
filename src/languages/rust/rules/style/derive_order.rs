use ra_ap_syntax::{AstNode, Edition, SourceFile, ast, syntax_editor::SyntaxEditor};

use crate::{AstCtx, Example, Violation};

const ADJACENT_PAIR_LEN: usize = 2;

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "sorted derives",
        code: "#[derive(Clone, Debug, thiserror::Error)]\nstruct Item;",
        pass: true,
    },
    Example {
        label: "unsorted derives",
        code: "#[derive(Debug, Clone)]\nstruct Item;",
        pass: false,
    },
    Example {
        label: "qualified paths use full text",
        code: "#[derive(thiserror::Error, Clone, Debug)]\nenum Failure {}",
        pass: false,
    },
    Example {
        label: "non-derive attribute",
        code: "#[cfg(test)]\nstruct Item;",
        pass: true,
    },
];

crate::ast_tree_rule!(
    derive_order,
    "Require traits inside derive attributes to be sorted alphabetically.",
    "Stable derive ordering keeps attribute diffs deterministic and makes duplicated traits easy to spot.",
    Low,
    fix_derive_order,
);

fn check_derive_order(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::Attr>()
        .filter_map(|attr| {
            let entries = derive_entries(&attr)?;

            (!is_sorted(&entries)).then(|| {
                ctx.violation(
                    &attr,
                    "derive traits are not alphabetically sorted (case-sensitive)",
                )
            })
        })
        .collect()
}

fn derive_entries(attr: &ast::Attr) -> Option<Vec<String>> {
    let (name, tokens) = attr.as_simple_call()?;

    if name != "derive" {
        return None;
    }

    let text = tokens.syntax().text().to_string();
    let body = text.strip_prefix('(')?.strip_suffix(')')?;

    Some(
        body.split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

fn is_sorted(entries: &[String]) -> bool {
    entries
        .windows(ADJACENT_PAIR_LEN)
        .all(|pair| pair[0] <= pair[1])
}

fn fix_derive_order(ctx: &AstCtx<'_>, _violations: &[Violation]) -> Option<String> {
    let (editor, root) = SyntaxEditor::with_ast_node(ctx.root);
    let mut changed = false;

    for attr in root.syntax().descendants().filter_map(ast::Attr::cast) {
        let Some(mut entries) = derive_entries(&attr) else {
            continue;
        };

        if is_sorted(&entries) {
            continue;
        }

        entries.sort();
        let replacement = parse_attr(&format!("#[derive({})]", entries.join(", ")))?;

        editor.replace(attr.syntax().clone(), replacement.syntax().clone());
        changed = true;
    }

    changed.then(|| editor.finish().new_root().to_string())
}

fn parse_attr(source: &str) -> Option<ast::Attr> {
    let fixture = format!("{source}\nstruct Fixture;");
    let parse = SourceFile::parse(&fixture, Edition::Edition2024);

    parse
        .errors()
        .is_empty()
        .then(|| parse.tree())?
        .syntax()
        .descendants()
        .find_map(ast::Attr::cast)
}

crate::rulewright_ast_test!(check_derive_order, {
    crate::example_tests!(EXAMPLES, check_derive_order);
    crate::fix_tests!(EXAMPLES, ast_tree, check_derive_order, fix_derive_order);
});
