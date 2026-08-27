use crate::{Example, FileCtx, Fix, Violation, infra::parse, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "deny warnings",
        code: "#![deny(warnings)]",
        pass: false,
    },
    Example {
        label: "deny unused",
        code: "#![deny(unused)]",
        pass: true,
    },
    Example {
        label: "comment with deny warnings",
        code: "// #![deny(warnings)]",
        pass: true,
    },
];

crate::line_rule!(
    deny_warnings,
    "Ban `#![deny(warnings)]` — breaks on compiler upgrades.",
    "deny(warnings) causes builds to break on compiler upgrades when new warnings are introduced. Use specific lint names.",
    Medium,
    fix_deny_warnings,
);

fn check_deny_warnings(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let mut out = Vec::new();

    for (i, line) in ctx.lines.iter().enumerate() {
        let lineno = i + 1;
        let trimmed = line.trim();

        if parse::is_comment(trimmed) {
            continue;
        }

        if crate::infra::helpers::contains_outside_strings(line, "deny(warnings)") {
            out.push(violation(
                ctx.rel,
                lineno,
                "`#![deny(warnings)]` breaks on compiler upgrades (use specific lint names)",
            ));
        }
    }

    out
}

fn fix_deny_warnings(ctx: &FileCtx<'_>, v: &Violation) -> Option<Fix> {
    let line = ctx.line(v.line)?;

    (line.trim() == "#![deny(warnings)]").then(|| Fix::delete(v.line, v.line))
}

crate::rulewright_test!(check_deny_warnings, {
    crate::example_tests!(EXAMPLES, check_deny_warnings);
    crate::fix_tests!(line, check_deny_warnings, fix_deny_warnings);
});
