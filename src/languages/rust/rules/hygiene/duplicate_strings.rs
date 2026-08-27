use std::collections::HashMap;

use crate::{Example, Violation, languages::workspace::WorkspaceCtx, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "long string repeated three times",
        code: "const A: &str = \"a deliberately long repeated fixture string value\";\nconst B: &str = \"a deliberately long repeated fixture string value\";\nconst C: &str = \"a deliberately long repeated fixture string value\";",
        pass: false,
    },
    Example {
        label: "only two occurrences",
        code: "const A: &str = \"a deliberately long repeated fixture string value\";\nconst B: &str = \"a deliberately long repeated fixture string value\";",
        pass: true,
    },
    Example {
        label: "short strings are exempt",
        code: "const A: &str = \"short\"; const B: &str = \"short\"; const C: &str = \"short\";",
        pass: true,
    },
];

crate::workspace_rule!(
    duplicate_strings,
    "Find long string literals repeated across files; full-workspace runs are authoritative.",
    "Repeated long literals should have one named source of truth or a shared fixture.",
    Low,
    params {
        min_chars: i64 = 40,
        min_occurrences: i64 = 3
    },
);

fn check_duplicate_strings(ctx: &WorkspaceCtx<'_>) -> Vec<Violation> {
    let min_chars = ctx.config.get_usize("rust_duplicate_strings", &PARAMS[0]);
    let min_occurrences = ctx.config.get_usize("rust_duplicate_strings", &PARAMS[1]);
    let mut occurrences = HashMap::<&str, Vec<(&str, usize)>>::default();

    for file in ctx.files {
        for string in &file.strings {
            let content = string
                .value
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .unwrap_or(&string.value);

            if content.chars().count() >= min_chars {
                occurrences
                    .entry(&string.value)
                    .or_default()
                    .push((&file.rel, string.line));
            }
        }
    }

    let mut violations = Vec::new();

    for locations in occurrences
        .values_mut()
        .filter(|locations| locations.len() >= min_occurrences)
    {
        locations.sort_unstable();
        let Some(&(first_rel, first_line)) = locations.first() else {
            continue;
        };

        for &(rel, line) in locations.iter().skip(1) {
            violations.push(violation(rel, line, format!("duplicate long string; hoist to a shared const (first at {first_rel}:{first_line})")));
        }
    }

    violations
}

crate::rulewright_workspace_test!(check_duplicate_strings, {
    crate::example_tests!(EXAMPLES, check_duplicate_strings);
});
