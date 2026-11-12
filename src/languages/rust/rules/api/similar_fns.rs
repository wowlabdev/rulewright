#[cfg(test)]
use googletest::prelude::*;
use std::collections::HashMap;

use crate::{
    Example, Violation,
    languages::workspace::{FunctionRecord, WorkspaceCtx},
    matches_ignore, violation,
};

const MIN_SHARED_SHINGLES: usize = 5;
const PERCENT_DENOMINATOR: usize = 100;
const CANDIDATE_PERCENT: usize = 10;

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "exact function bodies",
        code: "fn one(a: i32) -> i32 { let b = a + 1; let c = b + 2; let d = c + 3; let e = d + 4; let f = e + 5; let g = f + 6; let h = g + 7; let i = h + 8; let j = i + 9; let k = j + 10; k }\nfn two(a: i32) -> i32 { let b = a + 1; let c = b + 2; let d = c + 3; let e = d + 4; let f = e + 5; let g = f + 6; let h = g + 7; let i = h + 8; let j = i + 9; let k = j + 10; k }",
        pass: false,
    },
    Example {
        label: "near function bodies",
        code: "fn one(a: i32) -> i32 { let b = a + 1; let c = b + 2; let d = c + 3; let e = d + 4; let f = e + 5; let g = f + 6; let h = g + 7; let i = h + 8; let j = i + 9; let k = j + 10; k }\nfn two(a: i32) -> i32 { let b = a + 1; let c = b + 2; let d = c + 3; let e = d + 4; let f = e + 5; let g = f + 6; let h = g + 8; let i = h + 8; let j = i + 9; let k = j + 10; k }",
        pass: false,
    },
    Example {
        label: "short bodies are exempt",
        code: "fn one() -> i32 { 1 }\nfn two() -> i32 { 1 }",
        pass: true,
    },
];

crate::workspace_rule!(
    similar_fns,
    "Find exact and near duplicate function bodies; full-workspace runs are authoritative.",
    "Substantially identical functions may have drifted copies of one behavior. Share the implementation only when both functions must evolve together; keep coincidental similarity separate and suppress it with that reason. Each `uncompared` entry lists path groups separated by `|` (for example `crates/fast/** | crates/reference/**`); functions in different groups of one entry are not compared with each other, while each group is still compared within itself and with every other path.",
    Low,
    params {
        min_tokens: i64 = 40,
        jaccard_percent: i64 = 85,
        uncompared: [String] = []
    },
);

#[derive(Clone, Copy)]
struct Located<'a> {
    rel: &'a str,
    record: &'a FunctionRecord,
}

fn check_similar_fns(ctx: &WorkspaceCtx<'_>) -> Vec<Violation> {
    let records: Vec<Located<'_>> = ctx
        .files
        .iter()
        .filter(|file| !matches_ignore(&file.rel, ctx.config.ignore_patterns("rust_similar_fns")))
        .flat_map(|file| {
            file.functions
                .iter()
                .filter(|function| {
                    function.body_token_count
                        >= ctx.config.get_usize(
                            "rust_similar_fns",
                            &SIMILAR_FNS_PARAMS[0],
                            &file.rel,
                        )
                })
                .map(|record| Located {
                    rel: &file.rel,
                    record,
                })
        })
        .collect();
    let mut index = HashMap::<_, Vec<usize>>::default();

    for (record_index, located) in records.iter().enumerate() {
        for fingerprint in &located.record.body_shingles {
            index.entry(*fingerprint).or_default().push(record_index);
        }
    }

    let mut shared = HashMap::<(usize, usize), usize>::default();

    for members in index.values() {
        for (position, left) in members.iter().enumerate() {
            for right in members.iter().skip(position + 1) {
                *shared.entry((*left, *right)).or_default() += 1;
            }
        }
    }

    shared
        .into_iter()
        .filter_map(|((left_index, right_index), shared_count)| {
            let left = *records.get(left_index)?;
            let right = *records.get(right_index)?;
            let anchor_rel = left.rel.max(right.rel);
            let jaccard_percent =
                ctx.config
                    .get_usize("rust_similar_fns", &SIMILAR_FNS_PARAMS[1], anchor_rel);
            let uncompared =
                ctx.config
                    .get_str_array("rust_similar_fns", &SIMILAR_FNS_PARAMS[2], anchor_rel);

            if is_uncompared(&uncompared, left.rel, right.rel) {
                return None;
            }

            let left_shingles = &left.record.body_shingles;
            let right_shingles = &right.record.body_shingles;
            let smaller = left_shingles.len().min(right_shingles.len());
            let threshold = MIN_SHARED_SHINGLES.max(
                smaller
                    .saturating_mul(CANDIDATE_PERCENT)
                    .div_ceil(PERCENT_DENOMINATOR),
            );

            if shared_count < threshold {
                return None;
            }

            let exact = left.record.body_checksum == right.record.body_checksum;
            let union = left_shingles
                .len()
                .saturating_add(right_shingles.len())
                .saturating_sub(shared_count);
            let near = shared_count.saturating_mul(PERCENT_DENOMINATOR)
                >= union.saturating_mul(jaccard_percent);

            if !exact && !near {
                return None;
            }

            let (anchor, counterpart) =
                if (left.rel, left.record.line) > (right.rel, right.record.line) {
                    (left, right)
                } else {
                    (right, left)
                };

            Some(violation(
                anchor.rel,
                anchor.record.line,
                format!(
                    "{} function clone ({} shared shingles); similar to {}:{} ({})",
                    if exact { "exact" } else { "near" },
                    shared_count,
                    counterpart.rel,
                    counterpart.record.line,
                    counterpart.record.name
                ),
            ))
        })
        .collect()
}

/// Validate `uncompared` entries: two or more nonempty `|`-separated glob groups.
pub(crate) fn validate_uncompared(entries: &[String]) -> Result<(), String> {
    for entry in entries {
        let groups = entry.split('|').map(str::trim).collect::<Vec<_>>();

        if groups.len() < 2 || groups.iter().any(|group| group.is_empty()) {
            return Err(format!(
                "invalid uncompared entry `{entry}`: expected `glob | glob` with two or more groups"
            ));
        }

        for group in groups {
            crate::glob::validate(group)?;
        }
    }

    Ok(())
}

fn is_uncompared(entries: &[String], left: &str, right: &str) -> bool {
    entries.iter().any(|entry| {
        let groups = entry.split('|').map(str::trim).collect::<Vec<_>>();
        let left_groups = groups
            .iter()
            .map(|group| matches_ignore(left, &[*group]))
            .collect::<Vec<_>>();
        let right_groups = groups
            .iter()
            .map(|group| matches_ignore(right, &[*group]))
            .collect::<Vec<_>>();
        let shared = left_groups
            .iter()
            .zip(&right_groups)
            .any(|(left, right)| *left && *right);

        !shared && left_groups.contains(&true) && right_groups.contains(&true)
    })
}

crate::rulewright_workspace_test!(check_similar_fns, {
    crate::example_tests!(EXAMPLES, check_similar_fns);

    #[gtest]
    fn configured_test_paths_are_exempt() -> Result<()> {
        let source = EXAMPLES[0].code;
        let violations = crate::test_support::check_workspace_sources_with_ignore(
            &[("crates/example/src/tests.rs", source)],
            "rust_similar_fns",
            &["**/tests.rs"],
            check_similar_fns,
        );

        verify_true!(violations.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn uncompared_groups_skip_only_pairs_across_groups() -> Result<()> {
        let body = EXAMPLES[0].code.split('\n').next().or_fail()?;
        let mut config = crate::Config::generate_default(&[]);

        config.rules.insert(
            "rust_similar_fns".to_owned(),
            crate::infra::config::RuleConfig {
                enabled: true,
                ignore: Vec::new(),
                only: Vec::new(),
                overrides: Vec::new(),
                params: std::collections::BTreeMap::from([(
                    "uncompared".to_owned(),
                    toml::Value::Array(vec![toml::Value::String(
                        "fast/** | reference/**".to_owned(),
                    )]),
                )]),
            },
        );
        let violations = crate::testing::check_workspace_sources_with_config(
            &[
                ("fast/a.rs", body),
                ("fast/b.rs", body),
                ("reference/a.rs", body),
                ("shared/a.rs", body),
            ],
            &config,
            check_similar_fns,
        );
        let mut pairs = violations
            .iter()
            .map(|violation| {
                let counterpart = violation.message.split("similar to ").nth(1).unwrap_or("");

                (
                    violation.rel.clone(),
                    counterpart.split(':').next().unwrap_or("").to_owned(),
                )
            })
            .collect::<Vec<_>>();

        pairs.sort();

        verify_eq!(
            pairs,
            [
                ("fast/b.rs".to_owned(), "fast/a.rs".to_owned()),
                ("shared/a.rs".to_owned(), "fast/a.rs".to_owned()),
                ("shared/a.rs".to_owned(), "fast/b.rs".to_owned()),
                ("shared/a.rs".to_owned(), "reference/a.rs".to_owned()),
            ]
        )
    }

    #[gtest]
    fn uncompared_entries_need_two_valid_groups() -> Result<()> {
        verify_true!(validate_uncompared(&["a/** | b/**".to_owned()]).is_ok())?;
        verify_true!(validate_uncompared(&["a/**".to_owned()]).is_err())?;
        verify_true!(validate_uncompared(&["a/** |".to_owned()]).is_err())?;

        verify_true!(validate_uncompared(&["a/[x | b".to_owned()]).is_err())
    }
});
