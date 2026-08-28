#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasVisibility, VisibilityKind},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "balanced root",
        code: "pub struct Client;\npub mod account;\npub mod network;",
        pass: true,
    },
    Example {
        label: "empty root over many modules",
        code: "pub mod a;\npub mod b;\npub mod c;\npub mod d;\npub mod e;\npub mod f;\npub mod g;\npub mod h;",
        pass: false,
    },
    Example {
        label: "empty root with few modules",
        code: "pub mod a;\npub mod b;\npub mod c;\npub mod d;\npub mod e;\npub mod f;\npub mod g;",
        pass: true,
    },
    Example {
        label: "many private modules",
        code: "mod a;\nmod b;\nmod c;\nmod d;\nmod e;\nmod f;\nmod g;\nmod h;\npub use a::Client;",
        pass: true,
    },
    Example {
        label: "many modules with root item",
        code: "pub struct Client;\npub mod a;\npub mod b;\npub mod c;\npub mod d;\npub mod e;\npub mod f;\npub mod g;\npub mod h;",
        pass: true,
    },
];

crate::ast_rule!(
    unbalanced_crate_root,
    "Flag `lib.rs` roots that are flat item dumps (too many pub items) or empty shells (no pub items over many pub modules).",
    "A crate root with dozens of loose public items or nothing but module declarations is hard to navigate; balance essential items in the root with semantic submodules.",
    Low,
    params {
        max_root_items: i64 = 40,
        min_modules_for_empty_root: i64 = 8
    },
);

fn check_unbalanced_crate_root(ctx: &AstCtx<'_>) -> Vec<Violation> {
    if ctx.file.package_name.is_none()
        || ctx.file.path.file_name().and_then(|name| name.to_str()) != Some("lib.rs")
    {
        return Vec::new();
    }

    let max_root_items = ctx.file.config.get_usize(
        "rust_unbalanced_crate_root",
        &UNBALANCED_CRATE_ROOT_PARAMS[0],
    );
    let min_modules = ctx.file.config.get_usize(
        "rust_unbalanced_crate_root",
        &UNBALANCED_CRATE_ROOT_PARAMS[1],
    );
    let mut pub_items = 0usize;
    let mut pub_mods = 0usize;

    for item in ctx.root.syntax().children().filter_map(ast::Item::cast) {
        if ctx.is_in_test(&item) {
            continue;
        }

        match item {
            ast::Item::Module(module) if is_pub(module.visibility()) => pub_mods += 1,
            ast::Item::Use(_) | ast::Item::Module(_) => {}
            item if item_is_public(&item) => pub_items += 1,
            _ => {}
        }
    }

    if pub_items > max_root_items {
        vec![crate::violation(
            ctx.file.rel,
            1,
            format!(
                "crate root defines {pub_items} public items (max {max_root_items}) — group related items into modules"
            ),
        )]
    } else if pub_items == 0 && pub_mods >= min_modules {
        vec![crate::violation(
            ctx.file.rel,
            1,
            format!(
                "crate root has no public items but {pub_mods} public modules — hoist essential items into the root"
            ),
        )]
    } else {
        Vec::new()
    }
}

fn item_is_public(item: &ast::Item) -> bool {
    match item {
        ast::Item::Const(item) => is_pub(item.visibility()),
        ast::Item::Enum(item) => is_pub(item.visibility()),
        ast::Item::Fn(item) => is_pub(item.visibility()),
        ast::Item::Static(item) => is_pub(item.visibility()),
        ast::Item::Struct(item) => is_pub(item.visibility()),
        ast::Item::Trait(item) => is_pub(item.visibility()),
        ast::Item::TypeAlias(item) => is_pub(item.visibility()),
        ast::Item::Union(item) => is_pub(item.visibility()),
        _ => false,
    }
}

fn is_pub(visibility: Option<ast::Visibility>) -> bool {
    visibility.is_some_and(|vis| matches!(vis.kind(), VisibilityKind::Pub))
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;

    fn run_at(rel: &str, source: &str) -> Vec<Violation> {
        crate::test_support::check_source_ast_at(rel, source, check_unbalanced_crate_root)
    }

    #[gtest]
    fn examples() -> Result<()> {
        for ex in EXAMPLES {
            let violations = run_at("crates/demo/src/lib.rs", ex.code);

            verify_eq!(violations.is_empty(), ex.pass)?;
        }

        Ok(())
    }

    #[gtest]
    fn flat_root_over_limit_fails() -> Result<()> {
        let mut src = String::new();

        for i in 0..41 {
            let _ = writeln!(src, "pub fn f{i}() {{}}");
        }

        let v = run_at("crates/demo/src/lib.rs", &src);

        verify_eq!(v.len(), 1)?;
        verify_eq!(v[0].line, 1)?;

        Ok(())
    }

    #[gtest]
    fn flat_root_at_limit_passes() -> Result<()> {
        let mut src = String::new();

        for i in 0..40 {
            let _ = writeln!(src, "pub fn f{i}() {{}}");
        }

        let v = run_at("crates/demo/src/lib.rs", &src);

        verify_true!(v.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn non_lib_file_is_not_gated() -> Result<()> {
        let v = run_at(
            "crates/demo/src/util.rs",
            "pub mod a;\npub mod b;\npub mod c;\npub mod d;\npub mod e;\npub mod f;\npub mod g;\npub mod h;",
        );

        verify_true!(v.is_empty())?;

        Ok(())
    }
}
