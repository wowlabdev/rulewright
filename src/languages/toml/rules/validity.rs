use crate::{Example, TomlCtx, Violation, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "valid TOML",
        code: "name = \"example\"\n[table]\nvalue = 1\n",
        pass: true,
    },
    Example {
        label: "syntax error",
        code: "name =\n",
        pass: false,
    },
    Example {
        label: "duplicate key",
        code: "name = \"first\"\nname = \"second\"\n",
        pass: false,
    },
];

crate::toml_rule!(
    toml_validity,
    "Reject TOML syntax errors and semantic conflicts such as duplicate keys.",
    "Taplo validation catches malformed documents before language-specific consumers produce inconsistent diagnostics.",
    High,
);

fn check_toml_validity(ctx: &TomlCtx<'_>) -> Vec<Violation> {
    let mut violations: Vec<Violation> = ctx
        .parse
        .errors
        .iter()
        .map(|error| {
            violation(
                ctx.file.rel,
                ctx.line_of_offset(usize::from(error.range.start())),
                format!("TOML syntax error: {}", error.message),
            )
        })
        .collect();

    if ctx.parse.errors.is_empty()
        && let Err(errors) = ctx.dom.validate()
    {
        violations.extend(
            errors.map(|error| violation(ctx.file.rel, 1, format!("TOML semantic error: {error}"))),
        );
    }

    violations
}

crate::rulewright_toml_test!(check_toml_validity, {
    crate::example_tests!(EXAMPLES, check_toml_validity);
});
