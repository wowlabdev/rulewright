use crate::{Example, FileCtx, Violation, infra::parse, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "Box::leak without comment",
        code: "let x = Box::leak(Box::new(42));",
        pass: false,
    },
    Example {
        label: "Box::leak with SAFETY comment",
        code: "// SAFETY: static lifetime needed for FFI\nlet x = Box::leak(Box::new(42));",
        pass: true,
    },
    Example {
        label: "Box::leak with LEAK comment",
        code: "// LEAK: intentional for process lifetime\nlet x = Box::leak(Box::new(42));",
        pass: true,
    },
    Example {
        label: "comment line not flagged",
        code: "// Box::leak example",
        pass: true,
    },
    Example {
        label: "normal code",
        code: "let x = Box::new(42);",
        pass: true,
    },
];

crate::line_rule!(
    box_leak,
    "Require `SAFETY` or `LEAK` comment on `Box::leak()` calls.",
    "Box::leak deliberately gives up deallocation. Prefer owned storage with an explicit lifetime; when process-lifetime allocation is the design, document why the allocation count is bounded and why reclamation is unnecessary.",
    High,
);

const PATTERNS: &[&str] = &["Box::leak"];

fn check_box_leak(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let mut out = Vec::new();

    for (i, line) in ctx.lines.iter().enumerate() {
        let lineno = i + 1;
        let trimmed = line.trim();

        if parse::is_comment(trimmed) || parse::matches(trimmed, "/*") {
            continue;
        }

        for pattern in PATTERNS {
            if !trimmed.contains(pattern) {
                continue;
            }

            let has_justification = i
                .checked_sub(1)
                .and_then(|previous| ctx.lines.get(previous))
                .is_some_and(|previous| previous.contains("SAFETY") || previous.contains("LEAK"));

            if !has_justification {
                out.push(violation(
                    ctx.rel,
                    lineno,
                    format!(
                        "{pattern}() without a comment — add // SAFETY: or // LEAK: explaining why"
                    ),
                ));
            }
        }
    }

    out
}

crate::rulewright_test!(check_box_leak, {
    crate::example_tests!(EXAMPLES, check_box_leak);
});
