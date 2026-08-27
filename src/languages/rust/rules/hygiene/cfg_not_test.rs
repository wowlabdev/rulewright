use crate::{Example, FileCtx, Violation, infra::parse, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "cfg not test",
        code: "#[cfg(not(test))]",
        pass: false,
    },
    Example {
        label: "cfg test",
        code: "#[cfg(test)]",
        pass: true,
    },
    Example {
        label: "normal cfg",
        code: "#[cfg(feature = \"wasm\")]",
        pass: true,
    },
    Example {
        label: "comment with cfg not test",
        code: "// #[cfg(not(test))]",
        pass: true,
    },
];

crate::line_rule!(
    cfg_not_test,
    "Flag `#[cfg(not(test))]` — use dependency injection or feature flags instead.",
    "Code gated on #[cfg(not(test))] creates invisible production-only paths that are hard to test and reason about.",
    Medium,
);

fn check_cfg_not_test(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let mut out = Vec::new();

    for (i, line) in ctx.lines.iter().enumerate() {
        let lineno = i + 1;
        let trimmed = line.trim();

        if parse::is_comment(trimmed) {
            continue;
        }

        if crate::infra::helpers::contains_outside_strings(trimmed, "#[cfg(not(test))]")
            || crate::infra::helpers::contains_outside_strings(trimmed, "#![cfg(not(test))]")
        {
            out.push(violation(
                ctx.rel,
                lineno,
                "#[cfg(not(test))] — use dependency injection or feature flags instead",
            ));
        }
    }

    out
}

crate::rulewright_test!(check_cfg_not_test, {
    crate::example_tests!(EXAMPLES, check_cfg_not_test);
});
