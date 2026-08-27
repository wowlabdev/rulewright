use ra_ap_syntax::{
    AstNode,
    ast::{self, HasAttrs, HasDocComments, HasName, HasVisibility, VisibilityKind},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "undocumented pub fn",
        code: "pub fn foo() {}",
        pass: false,
    },
    Example {
        label: "documented pub fn",
        code: "/// Does something.\npub fn foo() {}",
        pass: true,
    },
    Example {
        label: "private fn",
        code: "fn foo() {}",
        pass: true,
    },
    Example {
        label: "pub(crate) fn",
        code: "pub(crate) fn foo() {}",
        pass: true,
    },
    Example {
        label: "doc hidden pub fn",
        code: "#[doc(hidden)]\npub fn foo() {}",
        pass: true,
    },
    Example {
        label: "undocumented pub struct",
        code: "pub struct S;",
        pass: false,
    },
    Example {
        label: "undocumented pub enum",
        code: "pub enum E { A }",
        pass: false,
    },
    Example {
        label: "undocumented pub trait",
        code: "pub trait T {}",
        pass: false,
    },
    Example {
        label: "undocumented pub type alias",
        code: "pub type A = u8;",
        pass: false,
    },
    Example {
        label: "undocumented pub const",
        code: "pub const C: u8 = 1;",
        pass: false,
    },
    Example {
        label: "undocumented pub static",
        code: "pub static S: u8 = 1;",
        pass: false,
    },
    Example {
        label: "documented pub struct",
        code: "/// A struct.\npub struct S;",
        pass: true,
    },
    Example {
        label: "pub(crate) struct not fully public",
        code: "pub(crate) struct S;",
        pass: true,
    },
    Example {
        label: "doc hidden pub struct",
        code: "#[doc(hidden)]\npub struct S;",
        pass: true,
    },
    Example {
        label: "doc name-value attr pub struct",
        code: "#[doc = \"x\"]\npub struct S;",
        pass: true,
    },
];

crate::ast_rule!(
    pub_api_docs,
    "Require doc comments on public items.",
    "Undocumented public items force users to read source code. Doc comments generate searchable API documentation.",
);

fn check_pub_api_docs(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let functions = ctx
        .nodes::<ast::Fn>()
        .filter_map(|item| missing_docs(ctx, &item, "function"));
    let structs = ctx
        .nodes::<ast::Struct>()
        .filter_map(|item| missing_docs(ctx, &item, "struct"));
    let enums = ctx
        .nodes::<ast::Enum>()
        .filter_map(|item| missing_docs(ctx, &item, "enum"));
    let traits = ctx
        .nodes::<ast::Trait>()
        .filter_map(|item| missing_docs(ctx, &item, "trait"));
    let aliases = ctx
        .nodes::<ast::TypeAlias>()
        .filter_map(|item| missing_docs(ctx, &item, "type alias"));
    let constants = ctx
        .nodes::<ast::Const>()
        .filter_map(|item| missing_docs(ctx, &item, "const"));
    let statics = ctx
        .nodes::<ast::Static>()
        .filter_map(|item| missing_docs(ctx, &item, "static"));

    functions
        .chain(structs)
        .chain(enums)
        .chain(traits)
        .chain(aliases)
        .chain(constants)
        .chain(statics)
        .collect()
}

fn missing_docs<T>(ctx: &AstCtx<'_>, item: &T, kind: &str) -> Option<Violation>
where
    T: AstNode + HasAttrs + HasDocComments + HasName + HasVisibility,
{
    if ctx.is_in_test(item)
        || !item
            .visibility()
            .is_some_and(|vis| matches!(vis.kind(), VisibilityKind::Pub))
        || item.doc_comments().next().is_some()
        || item
            .attrs()
            .any(|attr| attr.simple_name().as_deref() == Some("doc"))
    {
        return None;
    }

    let name = item.name()?;

    Some(ctx.violation(
        &name,
        format!("public {kind} `{name}` is missing documentation"),
    ))
}

crate::rulewright_ast_test!(check_pub_api_docs, {
    crate::example_tests!(EXAMPLES, check_pub_api_docs);
});
