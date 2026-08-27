use crate::{Example, FileCtx, Violation, infra::parse, violation};

/// Pass/fail cases for `example_tests!`.
#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "Parameters section",
        code: "/// # Parameters",
        pass: false,
    },
    Example {
        label: "Arguments section",
        code: "/// # Arguments",
        pass: false,
    },
    Example {
        label: "Params section at deeper level",
        code: "/// ## Params",
        pass: false,
    },
    Example {
        label: "inner doc Arguments section",
        code: "//! # Arguments",
        pass: false,
    },
    Example {
        label: "lowercase heading",
        code: "/// # arguments",
        pass: false,
    },
    Example {
        label: "Examples section",
        code: "/// # Examples",
        pass: true,
    },
    Example {
        label: "Type Parameters heading is not a parameter table",
        code: "/// # Type Parameters",
        pass: true,
    },
    Example {
        label: "heading inside code fence",
        code: "/// ```text\n/// # Arguments\n/// ```",
        pass: true,
    },
    Example {
        label: "plain comment is not a doc comment",
        code: "// # Arguments",
        pass: true,
    },
    Example {
        label: "prose mention of arguments",
        code: "/// Takes two arguments and merges them.",
        pass: true,
    },
];

crate::line_rule!(
    doc_param_table,
    "Ban `# Parameters`/`# Arguments`/`# Params` sections in doc comments.",
    "Rust docs explain parameters in prose, not tables; parameter sections duplicate the signature.",
    Low,
);

/// Section names that indicate a parameter table.
const HEADINGS: &[&str] = &["parameters", "arguments", "params"];

fn check_doc_param_table(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let mut out = Vec::new();
    let mut in_fence = false;

    for (i, line) in ctx.lines.iter().enumerate() {
        let Some(raw) = parse::doc_comment_content(line.trim()) else {
            in_fence = false;
            continue;
        };
        let text = raw.trim();

        if parse::matches(text, "```") {
            in_fence = !in_fence;
            continue;
        }

        if in_fence || !is_param_heading(text) {
            continue;
        }

        out.push(violation(
            ctx.rel,
            i + 1,
            "explain parameters in prose, not a `# Parameters`-style doc section",
        ));
    }

    out
}

fn is_param_heading(text: &str) -> bool {
    let stripped = text.trim_start_matches('#');

    if stripped.len() == text.len() {
        return false;
    }

    let name = stripped.trim();

    HEADINGS.iter().any(|h| name.eq_ignore_ascii_case(h))
}

crate::rulewright_test!(check_doc_param_table, {
    crate::example_tests!(EXAMPLES, check_doc_param_table);
});
