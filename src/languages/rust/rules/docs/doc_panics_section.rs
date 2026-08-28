use ra_ap_syntax::{
    AstNode,
    ast::{self, HasArgList, HasName, HasVisibility, VisibilityKind},
};

use super::support::{doc_lines, has_heading, is_item_or_impl_fn};
use crate::{AstCtx, Example, Violation};

/// Pass/fail cases for `example_tests!`.
#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "documented fn with unwrap and no Panics section",
        code: "/// Reads the value.\npub fn f(x: Option<u32>) -> u32 { x.unwrap() }",
        pass: false,
    },
    Example {
        label: "documented fn with unwrap and Panics section",
        code: "/// Reads the value.\n///\n/// # Panics\n/// Panics when `x` is `None`.\npub fn f(x: Option<u32>) -> u32 { x.unwrap() }",
        pass: true,
    },
    Example {
        label: "undocumented fn with unwrap",
        code: "pub fn f(x: Option<u32>) -> u32 { x.unwrap() }",
        pass: true,
    },
    Example {
        label: "documented fn without panic sources",
        code: "/// Adds one.\npub fn f(x: u32) -> u32 { x.saturating_add(1) }",
        pass: true,
    },
    Example {
        label: "expect counts as a panic source",
        code: "/// Reads the value.\npub fn f(x: Option<u32>) -> u32 { x.expect(\"present\") }",
        pass: false,
    },
    Example {
        label: "panic macro counts",
        code: "/// Never returns.\npub fn f() { panic!(\"boom\"); }",
        pass: false,
    },
    Example {
        label: "assert macros count",
        code: "/// Validates input.\npub fn f(x: u32, y: u32) { assert_eq!(x, y, \"mismatch\"); }",
        pass: false,
    },
    Example {
        label: "unreachable counts",
        code: "/// Dispatches.\npub fn f(x: bool) { if x { unreachable!(); } }",
        pass: true,
    },
    Example {
        label: "doc-hidden support API",
        code: "/// Test helper.\n#[doc(hidden)]\npub fn f(x: Option<u32>) -> u32 { x.expect(\"fixture invariant\") }",
        pass: true,
    },
    Example {
        label: "debug_assert does not count",
        code: "/// Validates input.\npub fn f(x: bool) { debug_assert!(x, \"must hold\"); }",
        pass: true,
    },
    Example {
        label: "private documented fn with unwrap",
        code: "/// Reads the value.\nfn f(x: Option<u32>) -> u32 { x.unwrap() }",
        pass: true,
    },
    Example {
        label: "impl fn with unwrap and no Panics section",
        code: "struct S;\nimpl S {\n    /// Reads the value.\n    pub fn f(&self, x: Option<u32>) -> u32 { x.unwrap() }\n}",
        pass: false,
    },
    Example {
        label: "unwrap inside nested item is not the fn's contract",
        code: "/// Delegates.\npub fn f() { fn inner() { None::<u32>.unwrap(); } }",
        pass: true,
    },
    Example {
        label: "test module fn",
        code: "#[cfg(test)]\nmod tests {\n    /// Reads the value.\n    pub fn f(x: Option<u32>) -> u32 { x.unwrap() }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    doc_panics_section,
    "Require a `# Panics` section on documented pub fns that can panic.",
    "A documented fn that may panic must state when under `# Panics`.",
    Medium,
);

/// Macro names whose invocation may panic at runtime.
const PANIC_MACROS: &[&str] = &["panic", "assert", "assert_eq", "assert_ne"];

fn check_doc_panics_section(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::Fn>()
        .filter(is_item_or_impl_fn)
        .filter(|function| !ctx.is_in_test(function) && is_fully_public(function))
        .filter(|function| !is_doc_hidden(function))
        .filter(can_panic)
        .filter_map(|function| {
            let docs = doc_lines(&function);

            if docs.is_empty() || has_heading(&docs, "Panics") {
                return None;
            }

            let name = function.name()?;

            Some(ctx.violation(
                &name,
                format!(
                    "documented `{}` can panic but its docs have no `# Panics` section",
                    name.text()
                ),
            ))
        })
        .collect()
}

fn is_doc_hidden(function: &ast::Fn) -> bool {
    use ra_ap_syntax::ast::HasAttrs;

    function.attrs().any(|attribute| {
        let source = attribute.syntax().text().to_string();

        source.contains("doc") && source.contains("hidden")
    })
}

fn is_fully_public(function: &ast::Fn) -> bool {
    function
        .visibility()
        .is_some_and(|visibility| matches!(visibility.kind(), VisibilityKind::Pub))
}

fn can_panic(function: &ast::Fn) -> bool {
    let Some(body) = function.body() else {
        return false;
    };
    let method_can_panic = body
        .syntax()
        .descendants()
        .filter_map(ast::MethodCallExpr::cast)
        .filter(|call| !inside_nested_item(call, function))
        .any(|call| {
            call.name_ref().is_some_and(|name| {
                name.text() == "expect"
                    || (name.text() == "unwrap"
                        && call
                            .arg_list()
                            .is_none_or(|args| args.args().next().is_none()))
            })
        });

    method_can_panic
        || body
            .syntax()
            .descendants()
            .filter_map(ast::MacroCall::cast)
            .filter(|call| !inside_nested_item(call, function))
            .any(|call| {
                let name = call
                    .path()
                    .and_then(|path| path.segment())
                    .and_then(|segment| segment.name_ref());

                name.is_some_and(|name| PANIC_MACROS.contains(&name.text().as_str()))
            })
}

fn inside_nested_item<N>(node: &N, function: &ast::Fn) -> bool
where
    N: AstNode,
{
    node.syntax()
        .ancestors()
        .skip(1)
        .take_while(|ancestor| ancestor != function.syntax())
        .any(|ancestor| ast::Item::can_cast(ancestor.kind()))
}

crate::rulewright_ast_test!(check_doc_panics_section, {
    crate::example_tests!(EXAMPLES, check_doc_panics_section);
});
