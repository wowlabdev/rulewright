use ra_ap_syntax::{
    AstNode,
    ast::{self, HasAttrs, HasDocComments, HasName, HasVisibility, LiteralKind, VisibilityKind},
};

use crate::{AstCtx, Example, Violation};

/// Pass/fail cases for `example_tests!`.
#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "undocumented const with numeric literal",
        code: "const TIMEOUT_SECS: u64 = 30;",
        pass: false,
    },
    Example {
        label: "undocumented static with string literal",
        code: "static ENDPOINT: &str = \"primary\";",
        pass: false,
    },
    Example {
        label: "literal nested in a call",
        code: "const RETRY: Duration = Duration::from_secs(30);",
        pass: false,
    },
    Example {
        label: "doc comment explains the value",
        code: "/// Upstream aborts after thirty seconds.\nconst TIMEOUT_SECS: u64 = 30;",
        pass: true,
    },
    Example {
        label: "line comment above explains the value",
        code: "// Matches the upstream timeout policy.\nconst TIMEOUT_SECS: u64 = 30;",
        pass: true,
    },
    Example {
        label: "comment above the attribute",
        code: "// Fixture table kept verbatim.\n#[rustfmt::skip]\nconst NAMES: &[&str] = &[\"a\"];",
        pass: true,
    },
    Example {
        label: "pub const is pub_api_docs territory",
        code: "pub const TIMEOUT_SECS: u64 = 30;",
        pass: true,
    },
    Example {
        label: "pub(crate) const still needs a doc",
        code: "pub(crate) const TIMEOUT_SECS: u64 = 30;",
        pass: false,
    },
    Example {
        label: "no literal in initializer",
        code: "const SIZE: usize = std::mem::size_of::<u64>();",
        pass: true,
    },
    Example {
        label: "const in test module",
        code: "#[cfg(test)]\nmod tests {\n    const TIMEOUT_SECS: u64 = 30;\n}",
        pass: true,
    },
    Example {
        label: "function-local const is not module-level",
        code: "fn f() -> u64 {\n    const LOCAL: u64 = 30;\n    LOCAL\n}",
        pass: true,
    },
];

crate::ast_rule!(
    const_needs_doc,
    "Require a doc or line comment on private consts and statics holding literal values.",
    "Magic values need context: why the value was chosen and what depends on it.",
    Low,
);

fn check_const_needs_doc(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let mut violations = Vec::new();

    for item in ctx.nodes::<ast::Const>() {
        check_literal_item(ctx, &item, "const", item.body(), &mut violations);
    }

    for item in ctx.nodes::<ast::Static>() {
        check_literal_item(ctx, &item, "static", item.body(), &mut violations);
    }

    violations
}

fn check_literal_item<N>(
    ctx: &AstCtx<'_>,
    item: &N,
    kind: &str,
    body: Option<ast::Expr>,
    violations: &mut Vec<Violation>,
) where
    N: AstNode + HasAttrs + HasDocComments + HasName + HasVisibility,
{
    let Some(name) = item.name() else {
        return;
    };

    if is_exempt_item(ctx, item)
        || item
            .visibility()
            .is_some_and(|visibility| matches!(visibility.kind(), VisibilityKind::Pub))
        || has_docs(item)
        || body.is_none_or(|body| !contains_magic_literal(&body))
        || has_comment_above(ctx, item, &name)
    {
        return;
    }

    violations.push(ctx.violation(
        &name,
        format!(
            "{kind} `{}` holds a literal value without a doc or comment explaining it",
            name.text()
        ),
    ));
}

fn is_exempt_item<N>(ctx: &AstCtx<'_>, item: &N) -> bool
where
    N: AstNode,
{
    ctx.is_in_test(item)
        || item.syntax().ancestors().skip(1).any(|ancestor| {
            ast::Fn::can_cast(ancestor.kind())
                || ast::AssocItemList::can_cast(ancestor.kind())
                || ast::ExternItemList::can_cast(ancestor.kind())
        })
}

fn has_docs(item: &impl HasDocComments) -> bool {
    item.doc_comments().next().is_some()
        || item
            .attrs()
            .any(|attr| attr.simple_name().as_deref() == Some("doc"))
}

fn has_comment_above<N>(ctx: &AstCtx<'_>, item: &N, name: &ast::Name) -> bool
where
    N: AstNode + HasAttrs,
{
    let first_line = item
        .attrs()
        .next()
        .map_or_else(|| ctx.line_of(name), |attr| ctx.line_of(&attr));

    ctx.file
        .line(first_line.saturating_sub(1))
        .is_some_and(|line| line.trim().starts_with("//"))
}

fn contains_magic_literal(expr: &ast::Expr) -> bool {
    expr.syntax()
        .descendants()
        .filter_map(ast::Literal::cast)
        .any(|literal| {
            matches!(
                literal.kind(),
                LiteralKind::IntNumber(_) | LiteralKind::FloatNumber(_) | LiteralKind::String(_)
            )
        })
}

crate::rulewright_ast_test!(check_const_needs_doc, {
    crate::example_tests!(EXAMPLES, check_const_needs_doc);
});
