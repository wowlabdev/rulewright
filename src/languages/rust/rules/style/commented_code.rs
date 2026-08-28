use crate::{Example, FileCtx, Violation, infra::parse, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "single commented line passes",
        code: "// let x = 5;",
        pass: true,
    },
    Example {
        label: "two consecutive commented code lines",
        code: "// let x = 5;\n// let y = 10;",
        pass: false,
    },
    Example {
        label: "normal comments",
        code: "// This is a normal comment\n// explaining the code below",
        pass: true,
    },
    Example {
        label: "doc style comments",
        code: "// NOTE: this is important\n// SAFETY: we checked bounds",
        pass: true,
    },
    Example {
        label: "non-consecutive code comments",
        code: "// let x = 5;\nlet y = 10;\n// let z = 15;",
        pass: true,
    },
    Example {
        label: "mixed code and prose resets",
        code: "// let x = 5;\n// This is a sentence about something.\n// let y = 10;",
        pass: true,
    },
    Example {
        label: "prose with for keyword",
        code: "// Each job can have multiple inputs (one for defaults, one for overrides)\n// The order is determined by the configuration entries",
        pass: true,
    },
    Example {
        label: "prose with self reference",
        code: "// Construct an OperationContext from &self.state + &mut self.sink.\n// Used 10 times in the event loop; macro avoids repeating the struct literal.",
        pass: true,
    },
];

crate::line_rule!(
    commented_code,
    "Detect blocks of commented-out code (2+ consecutive lines).",
    "Commented-out implementation code is dead weight. Restore it as executable code or delete it and rely on version control; rewrite illustrative pseudocode as prose or a proper documentation example instead of disguising it as a stale statement.",
);

const STARTS_WITH_PATTERNS: &[&str] = &[
    "let ",
    "fn ",
    "pub fn ",
    "pub(",
    "struct ",
    "enum ",
    "impl ",
    "use ",
    "mod ",
    "return ",
    "self.",
    "Self::",
    "if let ",
    "match ",
    "for ",
    "while ",
    "loop {",
    "} else {",
    "} else if ",
    "assert!(",
    "assert_eq!(",
    "#[",
];

const CONTAINS_PATTERNS: &[&str] = &[
    ".await",
    ".unwrap()",
    ".push(",
    ".insert(",
    ".expect(",
    "println!(",
    "eprintln!(",
    "dbg!(",
];

const SKIP_PREFIXES: &[&str] = &[
    "---",
    "===",
    "SAFETY:",
    "NOTE:",
    "IMPORTANT:",
    "TODO",
    "FIXME",
    "HACK",
    "Step ",
    "rulewright",
    "rustfmt",
    "clippy",
];

fn check_commented_code(ctx: &FileCtx<'_>) -> Vec<Violation> {
    const MIN_CONSECUTIVE_CODE_LINES: usize = 2;

    let mut out = Vec::new();
    let mut consecutive = 0;

    for (i, line) in ctx.lines.iter().enumerate() {
        if let Some(raw) = parse::prose_comment_content(line) {
            let content = raw.trim();

            if content.is_empty() || SKIP_PREFIXES.iter().any(|p| parse::matches(content, *p)) {
                consecutive = 0;
                continue;
            }

            let looks_like_code = STARTS_WITH_PATTERNS
                .iter()
                .any(|p| parse::matches(content, *p))
                || CONTAINS_PATTERNS.iter().any(|p| content.contains(p));

            if looks_like_code {
                consecutive += 1;

                if consecutive >= MIN_CONSECUTIVE_CODE_LINES {
                    out.push(violation(
                        ctx.rel,
                        i + 1,
                        "commented-out code (delete it or use version control)",
                    ));
                    consecutive = 0;
                }
            } else {
                consecutive = 0;
            }
        } else {
            consecutive = 0;
        }
    }

    out
}

crate::rulewright_test!(check_commented_code, {
    crate::example_tests!(EXAMPLES, check_commented_code);
});
