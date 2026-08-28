#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{
    AstNode, SyntaxNode,
    ast::{self, HasAttrs, HasName, HasVisibility, VisibilityKind},
};
use std::collections::HashSet;

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "pub mod with pub use reexport",
        code: "pub mod db {\n    pub struct Connection;\n}\npub use db::Connection;",
        pass: false,
    },
    Example {
        label: "pub crate mod with pub use",
        code: "pub(crate) mod db {\n    pub struct Connection;\n}\npub use db::Connection;",
        pass: true,
    },
    Example {
        label: "pub mod with private use",
        code: "pub mod db {\n    pub struct Connection;\n}\nuse db::Connection;",
        pass: true,
    },
    Example {
        label: "self-prefixed reexport",
        code: "pub mod db;\npub use self::db::Connection;",
        pass: false,
    },
    Example {
        label: "grouped reexport",
        code: "pub mod db;\nmod other;\npub use {db::Connection, other::Helper};",
        pass: false,
    },
    Example {
        label: "renamed reexport",
        code: "pub mod db;\npub use db::Connection as Conn;",
        pass: false,
    },
    Example {
        label: "module reexported under new name",
        code: "pub mod db;\npub use db as database;",
        pass: false,
    },
    Example {
        label: "doc hidden module",
        code: "#[doc(hidden)]\npub mod internals;\npub use internals::Helper;",
        pass: true,
    },
    Example {
        label: "underscore module",
        code: "pub mod _private;\npub use _private::Helper;",
        pass: true,
    },
    Example {
        label: "reexport from private sibling",
        code: "mod db {\n    pub struct Connection;\n}\npub use db::Connection;",
        pass: true,
    },
    Example {
        label: "nested module siblings",
        code: "pub mod outer {\n    pub mod inner {\n        pub struct A;\n    }\n    pub use inner::A;\n}",
        pass: false,
    },
    Example {
        label: "reexport in test module",
        code: "#[cfg(test)]\nmod tests {\n    pub mod db {\n        pub struct Connection;\n    }\n    pub use db::Connection;\n}",
        pass: true,
    },
];

crate::ast_rule!(
    single_item_path,
    "Flag `pub use` re-exports that duplicate paths already public through a sibling `pub mod`.",
    "Items reachable through two public paths clutter the API and confuse navigation; make the module non-pub or drop the re-export.",
    Medium,
);

fn check_single_item_path(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let mut out = Vec::new();

    check_siblings(ctx, ctx.root.syntax(), &mut out);

    for module in ctx
        .nodes::<ast::Module>()
        .filter(|module| !ctx.is_in_test(module))
    {
        if let Some(list) = module.item_list() {
            check_siblings(ctx, list.syntax(), &mut out);
        }
    }

    out
}

fn check_siblings(ctx: &AstCtx<'_>, scope: &SyntaxNode, out: &mut Vec<Violation>) {
    let items: Vec<_> = scope.children().filter_map(ast::Item::cast).collect();
    let mods = public_mod_names(&items);

    if mods.is_empty() {
        return;
    }

    for item in &items {
        let ast::Item::Use(use_item) = item else {
            continue;
        };

        if !is_pub(use_item.visibility()) || ctx.is_in_test(use_item) {
            continue;
        }

        if let Some(tree) = use_item.use_tree() {
            check_use_tree(ctx, &tree, &mods, out);
        }
    }
}

fn public_mod_names(items: &[ast::Item]) -> HashSet<String> {
    let mut names = HashSet::default();

    for item in items {
        let ast::Item::Module(module) = item else {
            continue;
        };

        if !is_pub(module.visibility()) {
            continue;
        }

        let Some(name) = module.name().map(|name| name.text().to_string()) else {
            continue;
        };

        if !name.starts_with('_') && !is_doc_hidden(module) {
            names.insert(name);
        }
    }

    names
}

fn is_doc_hidden(module: &ast::Module) -> bool {
    module.attrs().any(|attr| {
        attr.simple_name().as_deref() == Some("doc")
            && attr.syntax().text().to_string().contains("hidden")
    })
}

fn check_use_tree(
    ctx: &AstCtx<'_>,
    tree: &ast::UseTree,
    mods: &HashSet<String>,
    out: &mut Vec<Violation>,
) {
    let mut stack = vec![tree.clone()];

    while let Some(tree) = stack.pop() {
        if let Some(list) = tree.use_tree_list() {
            stack.extend(list.use_trees());
            continue;
        }

        if let Some(path) = tree.path() {
            let mut current = Some(path);
            let mut origin = None;

            while let Some(path) = current {
                if let Some(name) = path.segment().and_then(|segment| segment.name_ref())
                    && name.text() != "self"
                {
                    origin = Some(name);
                }

                current = path.qualifier();
            }

            if let Some(name_ref) = origin {
                let name = name_ref.text();

                if mods.contains(name.as_str()) {
                    out.push(dual_path_violation(ctx, &tree, name.as_str()));
                }
            }
        }
    }
}

fn dual_path_violation(ctx: &AstCtx<'_>, tree: &ast::UseTree, ident: &str) -> Violation {
    ctx.violation(
        tree,
        format!(
            "`pub use` re-exports through `pub mod {ident}` — items become public via two paths; make the module non-pub or drop the re-export"
        ),
    )
}

fn is_pub(visibility: Option<ast::Visibility>) -> bool {
    visibility.is_some_and(|vis| matches!(vis.kind(), VisibilityKind::Pub))
}

crate::rulewright_ast_test!(check_single_item_path, {
    crate::example_tests!(EXAMPLES, check_single_item_path);

    #[gtest]
    fn separate_scopes_do_not_pair() -> Result<()> {
        let v = run("pub mod a {\n\
                 mod db {\n\
                     pub struct Connection;\n\
                 }\n\
                 pub use db::Connection;\n\
             }\n\
             pub mod db;");
        verify_true!(v.is_empty())?;

        Ok(())
    }
});
