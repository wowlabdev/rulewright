use ra_ap_syntax::{
    AstNode,
    ast::{self, HasAttrs, HasVisibility},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "pub glob reexport",
        code: "pub use internals::*;",
        pass: false,
    },
    Example {
        label: "pub crate glob reexport",
        code: "pub(crate) use internals::*;",
        pass: false,
    },
    Example {
        label: "glob inside group",
        code: "pub use internals::{helpers::*, Config};",
        pass: false,
    },
    Example {
        label: "private glob import",
        code: "fn f() { use internals::*; }",
        pass: true,
    },
    Example {
        label: "named reexports",
        code: "pub use internals::{Config, Helper};",
        pass: true,
    },
    Example {
        label: "windows hal forwarding",
        code: "#[cfg(windows)]\npub use windows_impl::*;",
        pass: true,
    },
    Example {
        label: "target os hal forwarding",
        code: "#[cfg(target_os = \"linux\")]\npub use linux_impl::*;",
        pass: true,
    },
    Example {
        label: "target arch hal forwarding",
        code: "#[cfg(target_arch = \"wasm32\")]\npub use wasm_impl::*;",
        pass: true,
    },
    Example {
        label: "unix hal forwarding",
        code: "#[cfg(unix)]\npub use unix_impl::*;",
        pass: true,
    },
    Example {
        label: "glob reexport in test module",
        code: "#[cfg(test)]\nmod tests {\n    pub use internals::*;\n}",
        pass: true,
    },
];

crate::ast_rule!(
    glob_reexport,
    "Flag `pub use foo::*` glob re-exports outside platform-cfg'd HAL forwarding.",
    "Glob re-exports silently widen the public surface and are unreviewable in diffs; re-export items individually.",
    Medium,
);

fn check_glob_reexport(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let public_uses = ctx
        .nodes::<ast::Use>()
        .filter(|item| !ctx.is_in_test(item) && item.visibility().is_some())
        .filter(|item| !has_platform_cfg(item));

    public_uses
        .flat_map(|item| {
            item.syntax()
                .descendants()
                .filter_map(ast::UseTree::cast)
                .filter(|tree| tree.star_token().is_some())
                .map(|tree| ctx.violation(&tree, "glob re-export — re-export items individually"))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Platform-forwarding cfg attributes sanction glob re-exports (HAL pattern).
fn has_platform_cfg(item: &ast::Use) -> bool {
    item.attrs().any(|attr| {
        if attr.simple_name().as_deref() != Some("cfg") {
            return false;
        }

        let text: String = attr
            .syntax()
            .text()
            .to_string()
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect();

        text == "#[cfg(windows)]"
            || text == "#[cfg(unix)]"
            || text.starts_with("#[cfg(target_os=")
            || text.starts_with("#[cfg(target_arch=")
    })
}

crate::rulewright_ast_test!(check_glob_reexport, {
    crate::example_tests!(EXAMPLES, check_glob_reexport);
});
