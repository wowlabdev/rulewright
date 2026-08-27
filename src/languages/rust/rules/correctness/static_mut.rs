use crate::{Example, FileCtx, Violation, infra::parse, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "static mut declaration",
        code: "static mut X: i32 = 0;",
        pass: false,
    },
    Example {
        label: "static Mutex",
        code: "static X: Mutex<i32> = Mutex::new(0);",
        pass: true,
    },
    Example {
        label: "comment with static mut",
        code: "// static mut X: i32 = 0;",
        pass: true,
    },
    Example {
        label: "static mut in string literal",
        code: r#"let msg = "static mut is dangerous";"#,
        pass: true,
    },
];

crate::line_rule!(
    static_mut,
    "Ban `static mut` declarations — use `AtomicT`, `Mutex`, or `OnceLock`.",
    "static mut is unsound in multithreaded code and deprecated. Use AtomicT, Mutex, or OnceLock instead.",
    High,
);

fn check_static_mut(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let mut out = Vec::new();

    for (i, line) in ctx.lines.iter().enumerate() {
        let lineno = i + 1;
        let trimmed = line.trim();

        if parse::is_comment(trimmed) {
            continue;
        }

        if crate::infra::helpers::contains_outside_strings(line, "static mut ") {
            out.push(violation(
                ctx.rel,
                lineno,
                "`static mut` is UB-prone (use AtomicT, Mutex, or OnceLock)",
            ));
        }
    }

    out
}

crate::rulewright_test!(check_static_mut, {
    crate::example_tests!(EXAMPLES, check_static_mut);
});
