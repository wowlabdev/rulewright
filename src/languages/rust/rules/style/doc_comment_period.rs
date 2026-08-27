#[cfg(test)]
use googletest::prelude::*;
use winnow::combinator::alt;

use crate::{Example, FileCtx, Violation, infra::parse, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "missing period",
        code: "/// Returns the value",
        pass: false,
    },
    Example {
        label: "with period",
        code: "/// Returns the value.",
        pass: true,
    },
    Example {
        label: "markdown header",
        code: "/// # Examples",
        pass: true,
    },
    Example {
        label: "empty doc comment",
        code: "///",
        pass: true,
    },
    Example {
        label: "code fence",
        code: "/// ```rust",
        pass: true,
    },
    Example {
        label: "inner doc missing period",
        code: "//! Module description",
        pass: false,
    },
    Example {
        label: "inner doc with period",
        code: "//! Module description.",
        pass: true,
    },
    Example {
        label: "ends with backtick",
        code: "/// Returns `None`",
        pass: true,
    },
    Example {
        label: "ends with colon",
        code: "/// The following:",
        pass: true,
    },
];

crate::line_rule!(
    doc_comment_period,
    "Require doc comments to end with proper punctuation.",
    "Doc comments are sentences. Ending with punctuation keeps generated rustdoc consistent and professional. Not auto-fixable: appending a dot to a line that continues on the next line breaks the sentence — rephrase so each line is a complete sentence, or shorten the comment to one line.",
    Low,
);

#[rustfmt::skip]
const ALLOWED_ENDINGS: &[char] = &[
    '.',
    ')',
    ':',
    '!',
    '?',
    '`',
    ']',
];

fn check_doc_comment_period(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let mut out = Vec::new();
    let mut in_code_fence = false;

    for (i, line) in ctx.lines.iter().enumerate() {
        let lineno = i + 1;
        let trimmed = line.trim();

        let Some(raw) = parse::doc_comment_content(trimmed) else {
            in_code_fence = false;
            continue;
        };
        let text = raw.trim();

        if parse::matches(text, "```") {
            in_code_fence = !in_code_fence;
            continue;
        }

        if in_code_fence || text.is_empty() {
            continue;
        }

        if parse::matches(text, '#') {
            continue;
        }

        if parse::matches(raw, alt(("  ", " \t"))) {
            continue;
        }

        if let Some(last) = text.chars().last()
            && !ALLOWED_ENDINGS.contains(&last)
        {
            out.push(violation(
                ctx.rel,
                lineno,
                "doc comment should end with `.`, `)`, `:`, `!`, `?`, `` ` ``, or `]` — do \
                     not just append a dot mid-sentence; rephrase so each line is a complete \
                     sentence, or shorten the comment to one line",
            ));
        }
    }

    out
}

crate::rulewright_test!(check_doc_comment_period, {
    crate::example_tests!(EXAMPLES, check_doc_comment_period);

    #[gtest]
    fn code_block_lines_are_skipped() -> Result<()> {
        let src = "/// Example:\n/// ```\n/// let x = foo();\n/// assert_eq!(x, 1);\n/// ```";
        let v = run(src);
        verify_true!(v.is_empty())?;

        Ok(())
    }
});
