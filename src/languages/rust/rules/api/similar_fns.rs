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
    "Clone detection identifies behavior that should usually be shared behind one implementation.",
    Low,
    params {
        min_tokens: i64 = 40,
        jaccard_percent: i64 = 85
    },
);

#[derive(Clone, Copy)]
struct Located<'a> {
    rel: &'a str,
    record: &'a FunctionRecord,
}

fn check_similar_fns(ctx: &WorkspaceCtx<'_>) -> Vec<Violation> {
    let min_tokens = ctx.config.get_usize("rust_similar_fns", &PARAMS[0]);
    let jaccard_percent = ctx.config.get_usize("rust_similar_fns", &PARAMS[1]);
    let records: Vec<Located<'_>> = ctx
        .files
        .iter()
        .filter(|file| !matches_ignore(&file.rel, ctx.config.ignore_patterns("rust_similar_fns")))
        .flat_map(|file| {
            file.functions
                .iter()
                .filter(|function| function.body_token_count >= min_tokens)
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
});
