use crate::{Example, FileCtx, Violation, infra::parse, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "bare TODO",
        code: "// TODO: fix this later",
        pass: false,
    },
    Example {
        label: "tracked TODO",
        code: "// TODO(#123) fix this",
        pass: true,
    },
    Example {
        label: "bare FIXME",
        code: "// FIXME this is broken",
        pass: false,
    },
    Example {
        label: "bare HACK",
        code: "// HACK: workaround",
        pass: false,
    },
    Example {
        label: "bare XXX",
        code: "// XXX",
        pass: false,
    },
    Example {
        label: "no comment",
        code: "let todo = 5;",
        pass: true,
    },
    Example {
        label: "inline TODO",
        code: "let x = 1; // TODO fix",
        pass: false,
    },
    Example {
        label: "tracked FIXME",
        code: "// FIXME(perf regression in v2)",
        pass: true,
    },
];

crate::line_rule!(
    todo,
    "Require TODO/FIXME/HACK/XXX to have parenthesized context.",
    "TODO without context (who, ticket, deadline) becomes permanent. Parenthesized context ensures accountability.",
);

const BANNED: &[&str] = &["TODO", "FIXME", "HACK", "XXX"];

fn check_todo(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let mut out = Vec::new();

    for (i, line) in ctx.lines.iter().enumerate() {
        let Some(comment) = parse::find_comment_start(line) else {
            continue;
        };

        for keyword in BANNED {
            if let Some(after) = parse::find_keyword_suffix(comment, keyword) {
                if parse::matches(after, '(') {
                    continue;
                }

                out.push(violation(
                    ctx.rel,
                    i + 1,
                    format!(
                        "{keyword} without tracking context \
                         (use {keyword}(#issue) or {keyword}(reason))"
                    ),
                ));
            }
        }
    }

    out
}

crate::rulewright_test!(check_todo, {
    crate::example_tests!(EXAMPLES, check_todo);
});
