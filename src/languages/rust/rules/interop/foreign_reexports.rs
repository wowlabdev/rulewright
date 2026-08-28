#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasAttrs, HasName},
};
use std::collections::HashSet;

use crate::{AstCtx, Example, Violation, infra::workspace::WorkspaceMembers};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "pub use of foreign crate item",
        code: "pub use serde_json::Value;",
        pass: false,
    },
    Example {
        label: "pub use re-exporting foreign crate root",
        code: "pub use rand;",
        pass: false,
    },
    Example {
        label: "foreign item in use group",
        code: "pub use {std::fmt::Debug, chrono::Utc};",
        pass: false,
    },
    Example {
        label: "renamed foreign re-export",
        code: "pub use serde_json::Value as JsonValue;",
        pass: false,
    },
    Example {
        label: "pub use of crate item",
        code: "pub use crate::foo::Bar;",
        pass: true,
    },
    Example {
        label: "pub use of self path",
        code: "pub use self::inner::Thing;",
        pass: true,
    },
    Example {
        label: "pub use of std item",
        code: "pub use std::fmt::Debug;",
        pass: true,
    },
    Example {
        label: "pub use of workspace crate",
        code: "pub use rulewright::RuleRegistry;",
        pass: true,
    },
    Example {
        label: "non-pub use of foreign crate",
        code: "use serde_json::Value;",
        pass: true,
    },
    Example {
        label: "pub(crate) use of foreign crate",
        code: "pub(crate) use serde_json::Value;",
        pass: true,
    },
    Example {
        label: "doc(hidden) re-export",
        code: "#[doc(hidden)]\npub use serde_json::Value;",
        pass: true,
    },
    Example {
        label: "re-export inside __private module",
        code: "mod __private {\n    pub use serde_json::Value;\n}",
        pass: true,
    },
    Example {
        label: "re-export inside _private module",
        code: "mod _private {\n    pub use serde_json::Value;\n}",
        pass: true,
    },
    Example {
        label: "re-export in test module",
        code: "#[cfg(test)]\nmod tests {\n    pub use serde_json::Value;\n}",
        pass: true,
    },
    Example {
        label: "re-export from local module",
        code: "mod inner {}\npub use inner::Thing;",
        pass: true,
    },
    Example {
        label: "re-export from local out-of-line module",
        code: "pub mod infra;\npub use infra::Config;",
        pass: true,
    },
    Example {
        label: "grouped re-export from local module",
        code: "mod inner {}\npub use inner::{Thing, Other};",
        pass: true,
    },
    Example {
        label: "grouped re-export from foreign crate",
        code: "pub use serde_json::{Map, Value};",
        pass: false,
    },
];

crate::ast_rule!(
    foreign_reexports,
    "Flag `pub use` re-exports of items from foreign crates.",
    "Re-exported foreign items blur type identity into aliases; users should depend on the defining crate directly.",
    Medium,
    params {
        allowed: [String] = []
    },
);

const BUILTIN_ROOTS: &[&str] = &["crate", "self", "super", "std", "core", "alloc"];

fn check_foreign_reexports(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let allowed = ctx
        .file
        .config
        .get_str_array("rust_foreign_reexports", &FOREIGN_REEXPORTS_PARAMS[0]);
    let members = ctx.file.config.workspace().members(ctx.file.path);
    let local_mods = local_modules(ctx);

    let public_uses = ctx
        .nodes::<ast::Use>()
        .filter(|item| !ctx.is_in_test(item))
        .filter(|item| !inside_private_module(item));

    public_uses
        .filter(|item| super::support::is_fully_public(item) && !is_doc_hidden(item))
        .flat_map(|item| {
            item.use_tree()
                .into_iter()
                .flat_map(collect_roots)
                .filter_map(|root| {
                    let name = root.text().to_string();

                    (!is_internal_root(&name, &members)
                        && !local_mods.contains(&name)
                        && !allowed.contains(&name))
                        .then(|| {
                            ctx.violation(
                                &root,
                                format!(
                                    "`pub use` re-exports foreign crate `{name}` — users should import it from `{name}` directly"
                                ),
                            )
                        })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn local_modules(ctx: &AstCtx<'_>) -> HashSet<String> {
    ctx.nodes::<ast::Module>()
        .filter_map(|module| module.name().map(|name| name.text().to_string()))
        .collect()
}

fn is_internal_root(root: &str, members: &WorkspaceMembers) -> bool {
    BUILTIN_ROOTS.contains(&root) || members.is_member_root(root)
}

fn collect_roots(tree: ast::UseTree) -> Vec<ast::NameRef> {
    let mut out = Vec::new();
    let mut stack = vec![tree];

    while let Some(tree) = stack.pop() {
        if let Some(path) = tree.path() {
            let names = super::support::path_names(path.clone());

            if let Some(root_name) = names.first()
                && let Some(root) = path
                    .syntax()
                    .descendants()
                    .filter_map(ast::NameRef::cast)
                    .find(|name| name.text() == root_name.as_str())
            {
                out.push(root);
            }
        } else if let Some(list) = tree.use_tree_list() {
            stack.extend(list.use_trees());
        }
    }

    out
}

fn is_doc_hidden(item: &ast::Use) -> bool {
    item.attrs().any(|attr| {
        attr.simple_name().is_some_and(|name| name == "doc")
            && attr
                .syntax()
                .descendants_with_tokens()
                .filter_map(ra_ap_syntax::NodeOrToken::into_token)
                .any(|token| token.text() == "hidden")
    })
}

fn inside_private_module(item: &ast::Use) -> bool {
    item.syntax()
        .ancestors()
        .skip(1)
        .filter_map(ast::Module::cast)
        .filter_map(|module| module.name())
        .any(|name| matches!(name.text().as_str(), "_private" | "__private"))
}

crate::rulewright_ast_test!(check_foreign_reexports, {
    crate::example_tests!(EXAMPLES, check_foreign_reexports);

    #[gtest]
    fn qualified_group_reports_the_outer_root_once() -> Result<()> {
        let violations = run("pub use serde_json::{Map, Value};");
        verify_eq!(violations.len(), 1)?;
        verify_true!(violations[0].message.contains("`serde_json`"))?;

        Ok(())
    }
});
