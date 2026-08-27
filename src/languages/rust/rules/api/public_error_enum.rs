use ra_ap_syntax::ast::{self, HasName, HasVisibility, VisibilityKind};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "pub error enum",
        code: "pub enum ParseError { Eof, Syntax }",
        pass: false,
    },
    Example {
        label: "pub error kind enum",
        code: "pub enum IoErrorKind { NotFound, Denied }",
        pass: false,
    },
    Example {
        label: "private kind enum",
        code: "enum ErrorKind { Io, Protocol }",
        pass: true,
    },
    Example {
        label: "pub(crate) error enum",
        code: "pub(crate) enum ParseError { Eof }",
        pass: true,
    },
    Example {
        label: "pub enum without error suffix",
        code: "pub enum Mode { Fast, Slow }",
        pass: true,
    },
    Example {
        label: "pub error struct",
        code: "pub struct ParseError { line: usize }",
        pass: true,
    },
    Example {
        label: "pub error enum in test module",
        code: "#[cfg(test)]\nmod tests {\n    pub enum ParseError { Eof }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    public_error_enum,
    "Flag `pub enum` named `*Error`/`*ErrorKind` — expose a situation-specific error struct with a private kind enum instead.",
    "A public error enum exposes every failure mode as breaking API surface; a struct wrapping a private kind enum keeps internal failure modes non-breaking.",
    Medium,
);

fn check_public_error_enum(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let public_enums = ctx
        .nodes::<ast::Enum>()
        .filter(|item| !ctx.is_in_test(item))
        .filter(|item| {
            item.visibility()
                .is_some_and(|vis| matches!(vis.kind(), VisibilityKind::Pub))
        });

    public_enums
        .filter_map(|item| {
            let name = item.name()?;
            let text = name.text().to_string();

            (text.ends_with("Error") || text.ends_with("ErrorKind")).then(|| {
                ctx.violation(
                    &name,
                    format!(
                        "public enum `{text}` — expose a situation-specific error struct with a private kind enum"
                    ),
                )
            })
        })
        .collect()
}

crate::rulewright_ast_test!(check_public_error_enum, {
    crate::example_tests!(EXAMPLES, check_public_error_enum);
});
