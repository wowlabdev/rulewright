use crate::{Example, FileCtx, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "file directive at top with blank line",
        code: "// #rw(file: rust_panic) startup binary\n\nuse std::io;",
        pass: true,
    },
    Example {
        label: "multiple file directives with different reasons",
        code: "// #rw(file: rust_panic) startup binary\n// #rw(file: rust_unwrap_in_lib) CLI error handling\n\nuse std::io;",
        pass: true,
    },
    Example {
        label: "file directive missing blank line before code",
        code: "// #rw(file: rust_panic) startup binary\nuse std::io;",
        pass: false,
    },
    Example {
        label: "file directive after imports",
        code: "use std::io;\n// #rw(file: rust_panic) startup binary\n\nfn main() {}",
        pass: false,
    },
    Example {
        label: "non-file directive anywhere is fine",
        code: "use std::io;\n// #rw(rust_panic) reason\npanic!(\"x\");",
        pass: true,
    },
    Example {
        label: "file with no directives",
        code: "use std::io;\nfn main() {}",
        pass: true,
    },
    Example {
        label: "file directive then doc comment without blank line fails",
        code: "// #rw(file: rust_panic) startup binary\n//! Module docs.\n\nuse std::io;",
        pass: false,
    },
    Example {
        label: "file directive then blank then doc comment passes",
        code: "// #rw(file: rust_panic) startup binary\n\n//! Module docs.\n\nuse std::io;",
        pass: true,
    },
    Example {
        label: "mergeable directives with same reason",
        code: "// #rw(file: rust_panic) startup binary\n// #rw(file: rust_unwrap_in_lib) startup binary\n\nuse std::io;",
        pass: false,
    },
    Example {
        label: "merged directives on one line",
        code: "// #rw(file: rust_panic, rust_unwrap_in_lib) startup binary\n\nuse std::io;",
        pass: true,
    },
];

crate::line_rule!(
    rulewright_directives,
    "Enforce file-wide #rw directives at top of file with a blank line separator.",
    "File-wide rulewright directives belong at the very top so they are immediately visible. A blank line after them visually separates configuration from code.",
);

const FILE_PREFIX: &str = "// #rw(file:";
const DIRECTIVE_PAIR_SIZE: usize = 2;

use winnow::token::take_until;

fn is_file_directive(trimmed: &str) -> bool {
    crate::infra::parse::matches(trimmed, FILE_PREFIX)
}

fn extract_reason(line: &str) -> &str {
    let trimmed = line.trim();
    let mut input = trimmed;

    if crate::infra::parse::try_parse(&mut input, take_until(0.., ")")).is_some()
        && crate::infra::parse::try_parse(&mut input, ")").is_some()
    {
        return input.trim();
    }

    ""
}

fn check_rulewright_directives(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let lines = ctx.lines;

    if lines.is_empty() {
        return Vec::new();
    }

    let file_directive_lines: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| is_file_directive(line.trim()).then_some(index))
        .collect();

    if file_directive_lines.is_empty() {
        return Vec::new();
    }

    let header_end = directive_header_end(lines);
    let mut violations = misplaced_directives(ctx.rel, &file_directive_lines, header_end);

    violations.extend(duplicate_reason_violations(
        ctx.rel,
        lines,
        &file_directive_lines,
    ));

    if let Some(violation) =
        missing_separator_violation(ctx.rel, lines, &file_directive_lines, header_end)
    {
        violations.push(violation);
    }

    violations
}

fn directive_header_end(lines: &[&str]) -> usize {
    lines
        .iter()
        .take_while(|line| {
            let trimmed = line.trim();

            trimmed.is_empty() || is_file_directive(trimmed)
        })
        .count()
}

fn misplaced_directives(rel: &str, directive_lines: &[usize], header_end: usize) -> Vec<Violation> {
    directive_lines
        .iter()
        .filter(|&&line_idx| line_idx >= header_end)
        .map(|&line_idx| {
            crate::violation(
                rel,
                line_idx + 1,
                "file-wide #rw directive must be at the top of the file, before imports and code",
            )
        })
        .collect()
}

fn duplicate_reason_violations(
    rel: &str,
    lines: &[&str],
    directive_lines: &[usize],
) -> Vec<Violation> {
    let mut violations = Vec::new();

    for window in directive_lines.windows(DIRECTIVE_PAIR_SIZE) {
        let &[a, b] = window else { continue };
        let consecutive = (a + 1..b).all(|i| {
            lines.get(i).is_some_and(|line| {
                let trimmed = line.trim();

                trimmed.is_empty() || is_file_directive(trimmed)
            })
        });

        if !consecutive {
            continue;
        }

        let Some((reason_a, reason_b)) = lines
            .get(a)
            .zip(lines.get(b))
            .map(|(a, b)| (extract_reason(a), extract_reason(b)))
        else {
            continue;
        };

        if !reason_a.is_empty() && reason_a == reason_b {
            violations.push(crate::violation(
                rel,
                b + 1,
                format!(
                    "file-wide #rw directives with the same reason should be merged into one line (reason: {reason_a})"
                ),
            ));
        }
    }

    violations
}

fn missing_separator_violation(
    rel: &str,
    lines: &[&str],
    directive_lines: &[usize],
    header_end: usize,
) -> Option<Violation> {
    let last_directive = directive_lines
        .iter()
        .copied()
        .filter(|&i| i < header_end)
        .max()?;
    let content_idx =
        lines
            .iter()
            .enumerate()
            .skip(last_directive + 1)
            .find_map(|(index, line)| {
                let trimmed = line.trim();

                (!trimmed.is_empty() && !is_file_directive(trimmed)).then_some(index)
            })?;
    let has_blank = (last_directive + 1..content_idx)
        .any(|index| lines.get(index).is_some_and(|line| line.trim().is_empty()));

    if has_blank {
        None
    } else {
        Some(crate::violation(
            rel,
            content_idx + 1,
            "missing blank line after file-wide #rw directives",
        ))
    }
}

crate::rulewright_test!(check_rulewright_directives, {
    crate::example_tests!(EXAMPLES, check_rulewright_directives);
});
