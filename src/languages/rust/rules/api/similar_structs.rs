#[cfg(test)]
use googletest::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::{
    Example, Violation,
    languages::workspace::{StructRecord, WorkspaceCtx},
    matches_ignore, violation,
};

const CANDIDATE_FIELD_ALLOWANCE: usize = 2;
const PERCENT_DENOMINATOR: usize = 100;

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "exact twins",
        code: "struct One { a: u32, b: String, c: bool, d: f64 }\nstruct Two { d: f64, c: bool, b: String, a: u32 }",
        pass: false,
    },
    Example {
        label: "near twins",
        code: "struct One { a: u32, b: String, c: bool, d: f64 }\nstruct Two { a: u32, b: String, c: bool, d: f64, e: usize }",
        pass: false,
    },
    Example {
        label: "containment twins",
        code: "struct One { a: u32, b: String, c: bool, d: f64 }\nstruct Two { a: u32, b: String, c: bool, d: f64, e: usize, f: usize }",
        pass: false,
    },
    Example {
        label: "input twin is sanctioned",
        code: "struct Request { a: u32, b: String, c: bool, d: f64 }\nstruct RequestInput { a: u32, b: String, c: bool, d: f64 }",
        pass: true,
    },
    Example {
        label: "generic arity differs",
        code: "struct One<T> { a: u32, b: String, c: bool, d: T }\nstruct Two<T, U> { a: u32, b: String, c: bool, d: T, marker: U }",
        pass: true,
    },
    Example {
        label: "below threshold",
        code: "struct One { a: u32, b: String, c: bool }\nstruct Two { a: u32, b: String, c: bool, d: f64 }",
        pass: true,
    },
];

crate::workspace_rule!(
    similar_structs,
    "Find exact, near, and containment duplicate named-field structs; full-workspace runs are authoritative.",
    "Structural duplication often signals a missing shared domain type; indexed candidate generation keeps the check scalable.",
    Low,
    params {
        min_fields: i64 = 4,
        jaccard_percent: i64 = 80
    },
);

#[derive(Clone, Copy)]
struct Located<'a> {
    rel: &'a str,
    record: &'a StructRecord,
}

// #rw(fn: rust_alloc_in_loop) indexed candidate and evidence sets own normalized field signatures
// #rw(fn: rust_cyclomatic_complexity) tier scoring deliberately evaluates exact, near, and containment evidence together
fn check_similar_structs(ctx: &WorkspaceCtx<'_>) -> Vec<Violation> {
    let min_fields = ctx.config.get_usize("rust_similar_structs", &PARAMS[0]);
    let jaccard_percent = ctx.config.get_usize("rust_similar_structs", &PARAMS[1]);
    let records: Vec<Located<'_>> = ctx
        .files
        .iter()
        .filter(|file| {
            !matches_ignore(
                &file.rel,
                ctx.config.ignore_patterns("rust_similar_structs"),
            )
        })
        .flat_map(|file| {
            file.structs.iter().map(|record| Located {
                rel: &file.rel,
                record,
            })
        })
        .collect();
    let mut index = HashMap::<&str, Vec<usize>>::default();

    for (record_index, located) in records.iter().enumerate() {
        for (name, _) in &located.record.fields {
            index.entry(name).or_default().push(record_index);
        }
    }

    let mut shared = HashMap::<(usize, usize), usize>::default();

    for members in index.values() {
        for (position, left) in members.iter().enumerate() {
            for right in members.iter().skip(position + 1) {
                let pair = if left < right {
                    (*left, *right)
                } else {
                    (*right, *left)
                };

                *shared.entry(pair).or_default() += 1;
            }
        }
    }

    let candidate_floor = min_fields.saturating_sub(CANDIDATE_FIELD_ALLOWANCE);
    let mut violations = Vec::new();

    for ((left_index, right_index), shared_names) in shared {
        if shared_names < candidate_floor {
            continue;
        }

        let Some(&left) = records.get(left_index) else {
            continue;
        };
        let Some(&right) = records.get(right_index) else {
            continue;
        };

        if left.record.generics_arity != right.record.generics_arity
            || sanctioned_input_twins(&left.record.name, &right.record.name)
        {
            continue;
        }

        let left_fields: HashSet<(String, String)> = left.record.fields.iter().cloned().collect();
        let right_fields: HashSet<(String, String)> = right.record.fields.iter().cloned().collect();
        let exact = left_fields == right_fields && left.record.name != right.record.name;
        let intersection = left_fields.intersection(&right_fields).count();
        let union = left_fields.union(&right_fields).count();
        let near = left_fields.len() >= min_fields
            && right_fields.len() >= min_fields
            && intersection.saturating_mul(PERCENT_DENOMINATOR)
                >= union.saturating_mul(jaccard_percent);
        let (small, large) = if left_fields.len() <= right_fields.len() {
            (&left_fields, &right_fields)
        } else {
            (&right_fields, &left_fields)
        };
        let containment = small.len() >= min_fields
            && small.is_subset(large)
            && large.len().saturating_sub(small.len()) <= CANDIDATE_FIELD_ALLOWANCE
            && small.len() < large.len();
        let evidence = if exact {
            Some(format!("exact: {} identical fields", left_fields.len()))
        } else if near {
            Some(format!(
                "near: {intersection}/{union} field Jaccard overlap"
            ))
        } else if containment {
            Some(format!(
                "containment: {} of {} fields are shared",
                small.len(),
                large.len()
            ))
        } else {
            None
        };
        let Some(evidence) = evidence else { continue };
        let (anchor, counterpart) = later(left, right);

        violations.push(violation(
            anchor.rel,
            anchor.record.line,
            format!(
                "{evidence}; similar to {}:{}",
                counterpart.rel, counterpart.record.line
            ),
        ));
    }

    violations
}

fn sanctioned_input_twins(left: &str, right: &str) -> bool {
    left.strip_suffix("Input") == Some(right) || right.strip_suffix("Input") == Some(left)
}

fn later<'a>(left: Located<'a>, right: Located<'a>) -> (Located<'a>, Located<'a>) {
    if (left.rel, left.record.line) > (right.rel, right.record.line) {
        (left, right)
    } else {
        (right, left)
    }
}

crate::rulewright_workspace_test!(check_similar_structs, {
    crate::example_tests!(EXAMPLES, check_similar_structs);

    #[gtest]
    fn compares_multiple_sources() -> Result<()> {
        let violations = crate::check_workspace_sources(
            &[
                ("a.rs", "struct One { a: u32, b: String, c: bool, d: f64 }"),
                ("b.rs", "struct Two { a: u32, b: String, c: bool, d: f64 }"),
            ],
            check_similar_structs,
        );
        verify_eq!(violations.len(), 1)?;
        verify_eq!(violations[0].rel, "b.rs")?;
        verify_true!(violations[0].message.contains("a.rs:1"))?;

        Ok(())
    }

    #[gtest]
    fn honors_configured_path_ignores() -> Result<()> {
        let violations = crate::test_support::check_workspace_sources_with_ignore(
            &[
                (
                    "crates/example/src/lib.rs",
                    "struct One { a: u32, b: String, c: bool, d: f64 }",
                ),
                (
                    "crates/example/src/tests.rs",
                    "struct Two { a: u32, b: String, c: bool, d: f64 }",
                ),
            ],
            "rust_similar_structs",
            &["**/tests.rs"],
            check_similar_structs,
        );

        verify_true!(violations.is_empty())?;

        Ok(())
    }
});
