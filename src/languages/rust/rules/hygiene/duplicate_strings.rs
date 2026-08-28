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
    Example {
        label: "lint reasons are metadata, not shareable values",
        code: "#[expect(dead_code, reason = \"a deliberately long repeated lint reason string\")] fn a() {}\n#[expect(dead_code, reason = \"a deliberately long repeated lint reason string\")] fn b() {}\n#[expect(dead_code, reason = \"a deliberately long repeated lint reason string\")] fn c() {}",
        pass: true,
    },
    Example {
        label: "lint reasons inside macro definitions remain metadata",
        code: "macro_rules! a { () => { #[expect(dead_code, reason = \"a deliberately long repeated lint reason string\")] fn a() {} } }\nmacro_rules! b { () => { #[expect(dead_code, reason = \"a deliberately long repeated lint reason string\")] fn b() {} } }\nmacro_rules! c { () => { #[expect(dead_code, reason = \"a deliberately long repeated lint reason string\")] fn c() {} } }",
        pass: true,
    },
    Example {
        label: "strings in tests are fixtures",
        code: "#[cfg(test)] mod tests { const A: &str = \"a deliberately long repeated fixture string value\"; const B: &str = \"a deliberately long repeated fixture string value\"; const C: &str = \"a deliberately long repeated fixture string value\"; }",
        pass: true,
    },
    Example {
        label: "strings in Rulewright test registration macros are fixtures",
        code: "rulewright::rulewright_toml_test_at!(\"a deliberately long repeated fixture path value\", check, { let _ = \"a deliberately long repeated fixture path value\"; let _ = \"a deliberately long repeated fixture path value\"; });",
        pass: true,
    },
];

crate::workspace_rule!(
    duplicate_strings,
    "Find long string literals repeated throughout production source; full-workspace runs are authoritative.",
    "Repeated long literals may represent one value that can drift. Extract a constant or shared fixture only when the occurrences have the same meaning; tune or suppress the rule when equal text is coincidental rather than inventing a false abstraction.",
    Low,
    params {
        min_chars: i64 = 40,
        min_occurrences: i64 = 3
    },
);

fn check_duplicate_strings(ctx: &WorkspaceCtx<'_>) -> Vec<Violation> {
    let min_chars = ctx
        .config
        .get_usize("rust_duplicate_strings", &DUPLICATE_STRINGS_PARAMS[0]);
    let min_occurrences = ctx
        .config
        .get_usize("rust_duplicate_strings", &DUPLICATE_STRINGS_PARAMS[1]);
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
        let occurrence_count = locations.len();

        for &(rel, line) in locations.iter().skip(1) {
            violations.push(violation(
                rel,
                line,
                format!(
                    "long string appears {occurrence_count} times (first at {first_rel}:{first_line}); extract one named source only when the occurrences represent the same value"
                ),
            ));
        }
    }

    violations
}

crate::rulewright_workspace_test!(check_duplicate_strings, {
    crate::example_tests!(EXAMPLES, check_duplicate_strings);
});
