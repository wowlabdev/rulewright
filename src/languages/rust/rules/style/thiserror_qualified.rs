use ra_ap_syntax::{
    AstNode, Edition, SourceFile,
    ast::{self, HasVisibility},
    syntax_editor::SyntaxEditor,
};

use super::super::support::parse_use;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "qualified derive",
        code: "#[derive(Debug, thiserror::Error)]\n#[error(\"failed\")]\nstruct Failure;",
        pass: true,
    },
    Example {
        label: "single bare derive",
        code: "use thiserror::Error;\n\n#[derive(Debug, Error)]\n#[error(\"failed\")]\nstruct Failure;",
        pass: false,
    },
    Example {
        label: "multiple bare derives",
        code: "use thiserror::Error;\n\n#[derive(Error)]\n#[error(\"one\")]\nstruct One;\n#[derive(Error)]\n#[error(\"two\")]\nstruct Two;",
        pass: false,
    },
    Example {
        label: "import retained for a type use",
        code: "use thiserror::Error;\n\n#[derive(Error)]\n#[error(\"one\")]\nstruct One;\nfn accepts(value: &dyn Error) {}",
        pass: false,
    },
    Example {
        label: "unrelated bare Error derive",
        code: "#[derive(Error)]\nstruct Failure;",
        pass: true,
    },
];

crate::ast_tree_rule!(
    thiserror_qualified,
    "Require thiserror derives to use the qualified `thiserror::Error` path.",
    "Qualified derives expose their provenance and avoid a file-wide trait import used only by attributes.",
    Low,
    fix_thiserror_qualified,
);

fn check_thiserror_qualified(ctx: &AstCtx<'_>) -> Vec<Violation> {
    if !ctx.nodes::<ast::Use>().any(|item| imported_error(&item)) {
        return Vec::new();
    }

    ctx.nodes::<ast::Attr>()
        .filter(|attr| {
            derive_entries(attr).is_some_and(|entries| entries.iter().any(|entry| entry == "Error"))
        })
        .map(|attr| {
            ctx.violation(
                &attr,
                "thiserror derives must use `thiserror::Error`, not bare `Error`",
            )
        })
        .collect()
}

fn derive_entries(attr: &ast::Attr) -> Option<Vec<String>> {
    let (name, tokens) = attr.as_simple_call()?;

    if name != "derive" {
        return None;
    }

    let text = tokens.syntax().text().to_string();

    Some(
        text.strip_prefix('(')?
            .strip_suffix(')')?
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

fn imported_error(item: &ast::Use) -> bool {
    let Some(tree) = item.use_tree() else {
        return false;
    };
    let Some(path) = tree.path() else {
        return false;
    };
    let path_text = path
        .syntax()
        .text()
        .to_string()
        .replace(char::is_whitespace, "");

    if path_text == "thiserror::Error" {
        return true;
    }

    path_text == "thiserror"
        && tree.use_tree_list().is_some_and(|list| {
            list.use_trees().any(|child| {
                child
                    .path()
                    .is_some_and(|path| path.syntax().text().to_string().trim() == "Error")
            })
        })
}

fn error_used_outside_import_or_derive(root: &SourceFile) -> bool {
    root.syntax()
        .descendants()
        .filter_map(ast::NameRef::cast)
        .filter(|name| name.text() == "Error")
        .any(|name| {
            !name.syntax().ancestors().any(|ancestor| {
                ast::Use::can_cast(ancestor.kind())
                    || ast::Attr::cast(ancestor).is_some_and(|attr| {
                        derive_entries(&attr)
                            .is_some_and(|entries| entries.iter().any(|entry| entry == "Error"))
                    })
            })
        })
}

#[expect(
    clippy::option_option,
    reason = "the three states distinguish unrelated imports, deleted imports, and replacement imports"
)]
fn replacement_import(item: &ast::Use) -> Option<Option<ast::Use>> {
    let tree = item.use_tree()?;
    let path = tree.path()?;
    let path_text = path
        .syntax()
        .text()
        .to_string()
        .replace(char::is_whitespace, "");

    if path_text == "thiserror::Error" {
        return Some(None);
    }

    if path_text != "thiserror" {
        return None;
    }

    let remaining: Vec<String> = tree
        .use_tree_list()?
        .use_trees()
        .filter(|child| {
            child
                .path()
                .is_none_or(|path| path.syntax().text().to_string().trim() != "Error")
        })
        .map(|child| child.syntax().text().to_string())
        .collect();

    if remaining.is_empty() {
        return Some(None);
    }

    let visibility = item
        .visibility()
        .map_or_else(String::new, |visibility| format!("{visibility} "));
    let source = format!("{visibility}use thiserror::{{{}}};", remaining.join(", "));

    parse_use(&source).map(Some)
}

fn fix_thiserror_qualified(ctx: &AstCtx<'_>, _violations: &[Violation]) -> Option<String> {
    let (editor, root) = SyntaxEditor::with_ast_node(ctx.root);
    let mut changed = false;

    for attr in root.syntax().descendants().filter_map(ast::Attr::cast) {
        let Some(mut entries) = derive_entries(&attr) else {
            continue;
        };
        let mut rewritten = false;

        for entry in &mut entries {
            if entry == "Error" {
                "thiserror::Error".clone_into(entry);
                rewritten = true;
            }
        }

        if rewritten {
            let replacement = parse_attr(&format!("#[derive({})]", entries.join(", ")))?;

            editor.replace(attr.syntax().clone(), replacement.syntax().clone_subtree());
            changed = true;
        }
    }

    if !error_used_outside_import_or_derive(&root) {
        for item in root.syntax().descendants().filter_map(ast::Use::cast) {
            if !imported_error(&item) {
                continue;
            }

            match replacement_import(&item)? {
                Some(replacement) => {
                    editor.replace(item.syntax().clone(), replacement.syntax().clone_subtree());
                }

                None => editor.delete(item.syntax().clone()),
            }

            changed = true;
        }
    }

    changed.then(|| editor.finish().new_root().to_string())
}

fn parse_attr(source: &str) -> Option<ast::Attr> {
    let parse = SourceFile::parse(&format!("{source}\nstruct Fixture;"), Edition::Edition2024);

    parse
        .errors()
        .is_empty()
        .then(|| parse.tree())?
        .syntax()
        .descendants()
        .find_map(ast::Attr::cast)
}

crate::rulewright_ast_test!(check_thiserror_qualified, {
    crate::example_tests!(EXAMPLES, check_thiserror_qualified);
    crate::fix_tests!(
        EXAMPLES,
        ast_tree,
        check_thiserror_qualified,
        fix_thiserror_qualified
    );
});
