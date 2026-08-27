use crate::{
    Example, FileCtx, Violation,
    infra::{helpers, parse, scanner},
    violation,
};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "outer allow",
        code: "#[allow(dead_code)]\nfn f() {}",
        pass: false,
    },
    Example {
        label: "inner allow",
        code: "#![allow(clippy::too_many_arguments)]",
        pass: false,
    },
    Example {
        label: "allow with comment still flagged",
        code: "#[allow(dead_code)] // webhook response fields",
        pass: false,
    },
    Example {
        label: "expect with reason",
        code: "#[expect(dead_code, reason = \"kept for wire format\")]\nfn f() {}",
        pass: true,
    },
    Example {
        label: "allow inside macro_rules body",
        code: "macro_rules! m {\n    () => {\n        #[allow(unused)]\n        fn f() {}\n    };\n}",
        pass: true,
    },
    Example {
        label: "allow after macro_rules closed",
        code: "macro_rules! m {\n    () => {};\n}\n#[allow(unused)]\nfn f() {}",
        pass: false,
    },
    Example {
        label: "allow in comment",
        code: "// #[allow(dead_code)]",
        pass: true,
    },
    Example {
        label: "allow in string literal",
        code: "fn f() { let s = \"#[allow(dead_code)]\"; }",
        pass: true,
    },
];

crate::line_rule!(
    expect_over_allow,
    "Flag `#[allow(...)]` in hand-written code — use `#[expect(..., reason = \"...\")]` instead.",
    "#[expect] warns when the suppressed lint no longer fires, preventing stale suppressions from accumulating; #[allow] silences forever (clippy analogue: allow_attributes).",
    Medium,
);

fn check_expect_over_allow(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let mut out = Vec::new();
    let mut in_macro = false;
    let mut brace_depth: usize = 0;
    let mut seen_brace = false;

    for (i, line) in ctx.lines.iter().enumerate() {
        let trimmed = line.trim();

        if parse::is_comment(trimmed) {
            continue;
        }

        let code = scanner::code_only(line);

        if !in_macro && code.contains("macro_rules!") {
            in_macro = true;
            brace_depth = 0;
            seen_brace = false;
        }

        if in_macro {
            for ch in code.chars() {
                match ch {
                    '{' => {
                        brace_depth += 1;
                        seen_brace = true;
                    }
                    '}' => brace_depth = brace_depth.saturating_sub(1),
                    _ => {}
                }
            }

            if seen_brace && brace_depth == 0 {
                in_macro = false;
            }

            continue;
        }

        if helpers::contains_outside_strings(line, "#[allow(")
            || helpers::contains_outside_strings(line, "#![allow(")
        {
            out.push(violation(
                ctx.rel,
                i + 1,
                "#[allow(...)] — use #[expect(..., reason = \"...\")] so stale suppressions warn",
            ));
        }
    }

    out
}

crate::rulewright_test!(check_expect_over_allow, {
    crate::example_tests!(EXAMPLES, check_expect_over_allow);
});
