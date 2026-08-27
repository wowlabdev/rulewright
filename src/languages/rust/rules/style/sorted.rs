#[cfg(test)]
use googletest::prelude::*;

use crate::{Example, FileCtx, Fix, Violation, infra::parse, violation};

const ASC_MARKER: &str = "// #rw:sorted(asc)";
const DESC_MARKER: &str = "// #rw:sorted(desc)";
const MIN_SORTABLE_LINES: usize = 2;

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "ascending region",
        code: "// #rw:sorted(asc)\nuse alpha;\nuse beta;\nuse gamma;",
        pass: true,
    },
    Example {
        label: "unsorted ascending region",
        code: "// #rw:sorted(asc)\nuse gamma;\nuse alpha;\nuse beta;",
        pass: false,
    },
    Example {
        label: "descending region",
        code: "// #rw:sorted(desc)\nuse gamma;\nuse beta;\nuse alpha;",
        pass: true,
    },
    Example {
        label: "no marker",
        code: "use zebra;\nuse alpha;",
        pass: true,
    },
];

crate::full_line_rule!(
    sorted,
    "Enforce ordering in contiguous regions marked with `#rw:sorted(asc)` or `#rw:sorted(desc)`.",
    "Sorted lists prevent merge conflicts and make entries easy to locate.",
    Low,
    fix_sorted,
);

#[derive(Clone, Copy)]
enum Direction {
    Ascending,
    Descending,
}

impl Direction {
    const fn label(self) -> &'static str {
        match self {
            Self::Ascending => "ascending",
            Self::Descending => "descending",
        }
    }
}

fn sort_key(line: &str) -> String {
    use winnow::combinator::alt;

    let mut input = line.trim();
    let _ = parse::try_parse(&mut input, alt(("pub(crate) ", "pub(super) ", "pub ")));
    let _ = parse::try_parse(&mut input, alt(("extern crate ", "use ", "mod ")));

    input.to_lowercase()
}

fn check_sorted(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let comment_starts = crate::infra::scanner::line_comment_starts(ctx.contents);
    let mut violations = Vec::new();

    for (marker, direction) in sorted_regions(ctx, &comment_starts) {
        let (_, end) = region_bounds(ctx.lines, marker);
        let mut previous: Option<String> = None;

        for (index, line) in ctx.lines.iter().enumerate().take(end).skip(marker + 1) {
            let trimmed = line.trim();

            if parse::is_comment(trimmed) {
                continue;
            }

            let key = sort_key(trimmed);
            let ordered = previous.as_ref().is_none_or(|previous| match direction {
                Direction::Ascending => previous <= &key,
                Direction::Descending => previous >= &key,
            });

            if !ordered {
                violations.push(violation(
                    ctx.rel,
                    index + 1,
                    format!(
                        "line is not in {} order (sorted region starts at line {})",
                        direction.label(),
                        marker + 1
                    ),
                ));

                break;
            }

            previous = Some(key);
        }
    }

    violations
}

fn sorted_regions(ctx: &FileCtx<'_>, comment_starts: &[Option<usize>]) -> Vec<(usize, Direction)> {
    ctx.lines
        .iter()
        .enumerate()
        .filter_map(|(index, _)| {
            marker_direction(ctx, index, comment_starts).map(|direction| (index, direction))
        })
        .collect()
}

fn marker_direction(
    ctx: &FileCtx<'_>,
    index: usize,
    comment_starts: &[Option<usize>],
) -> Option<Direction> {
    let line = ctx.lines.get(index)?;

    if super::marked_region::is_source_marker(line, index, comment_starts, ASC_MARKER) {
        return Some(Direction::Ascending);
    }

    super::marked_region::is_source_marker(line, index, comment_starts, DESC_MARKER)
        .then_some(Direction::Descending)
}

fn region_bounds(lines: &[&str], marker: usize) -> (usize, usize) {
    let start = marker.saturating_add(1);
    let end = lines
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(index, line)| region_terminator(line).then_some(index))
        .unwrap_or(lines.len());

    (start, end)
}

fn region_terminator(line: &str) -> bool {
    let trimmed = line.trim();

    super::marked_region::ends_region(line) || trimmed.starts_with("// #rw:")
}

fn fix_sorted(ctx: &FileCtx<'_>, violation: &Violation) -> Option<Fix> {
    let comment_starts = crate::infra::scanner::line_comment_starts(ctx.contents);
    let (marker, direction) =
        sorted_regions(ctx, &comment_starts)
            .into_iter()
            .find(|(marker, _)| {
                let (start, end) = region_bounds(ctx.lines, *marker);

                (start + 1..=end).contains(&violation.line)
            })?;
    let (start, end) = region_bounds(ctx.lines, marker);
    let result = sorted_region(ctx.lines.get(start..end)?, direction)?;

    Some(Fix {
        start_line: start + 1,
        end_line: end,
        replacement: result.join("\n"),
    })
}

fn sorted_region<'a>(lines: &[&'a str], direction: Direction) -> Option<Vec<&'a str>> {
    let mut sorted: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| !parse::is_comment(line.trim()))
        .collect();

    if sorted.len() < MIN_SORTABLE_LINES {
        return None;
    }

    sorted.sort_by_key(|line| sort_key(line));

    if matches!(direction, Direction::Descending) {
        sorted.reverse();
    }

    let mut sorted = sorted.into_iter();

    lines
        .iter()
        .map(|line| {
            if parse::is_comment(line.trim()) {
                Some(*line)
            } else {
                sorted.next()
            }
        })
        .collect()
}

crate::rulewright_test!(check_sorted, {
    crate::example_tests!(EXAMPLES, check_sorted);
    crate::fix_tests!(line, check_sorted, fix_sorted);

    #[gtest]
    fn comments_keep_their_slots() -> Result<()> {
        let source = "// #rw:sorted(asc)\nuse gamma;\n// group boundary\nuse alpha;\nuse beta;";
        let expected = "// #rw:sorted(asc)\nuse alpha;\n// group boundary\nuse beta;\nuse gamma;";

        verify_eq!(
            crate::apply_line_fixes(source, check_sorted, fix_sorted),
            expected
        )
    }

    #[gtest]
    fn blank_line_ends_the_region() -> Result<()> {
        let source = "// #rw:sorted(asc)\nuse alpha;\nuse beta;\n\nuse zebra;\nuse aardvark;";

        verify_true!(run(source).is_empty())
    }

    #[gtest]
    fn strips_visibility_and_declaration_prefixes() -> Result<()> {
        let source = "// #rw:sorted(asc)\npub mod alpha;\npub(crate) mod beta;\nmod gamma;";

        verify_true!(run(source).is_empty())
    }

    #[gtest]
    fn markers_inside_raw_strings_are_ignored() -> Result<()> {
        let source = r##"const SOURCE: &str = r#"
// #rw:sorted(asc)
use zebra;
use alpha;
"#;"##;

        verify_true!(run(source).is_empty())
    }

    #[gtest]
    fn marker_requires_at_least_two_sortable_lines() -> Result<()> {
        verify_true!(run("// #rw:sorted(asc)\nuse alpha;").is_empty())
    }

    #[gtest]
    fn one_region_produces_one_finding_and_one_fix() -> Result<()> {
        let source = "// #rw:sorted(asc)\nuse delta;\nuse charlie;\nuse beta;\nuse alpha;";
        let violations = run(source);
        let fixed = crate::apply_line_fixes(source, check_sorted, fix_sorted);

        verify_eq!(violations.len(), 1)?;
        verify_eq!(
            fixed,
            "// #rw:sorted(asc)\nuse alpha;\nuse beta;\nuse charlie;\nuse delta;"
        )
    }

    #[gtest]
    fn composite_closers_and_following_code_are_never_sorted() -> Result<()> {
        for closer in [
            "]);",
            "]),",
            "),",
            "));",
            "]); // values",
            "]); /* values */",
        ] {
            let source =
                format!("// #rw:sorted(asc)\nValue::Zulu,\nValue::Alpha,\n{closer}\nafter();");
            let expected =
                format!("// #rw:sorted(asc)\nValue::Alpha,\nValue::Zulu,\n{closer}\nafter();");

            verify_eq!(
                crate::apply_line_fixes(&source, check_sorted, fix_sorted),
                expected
            )?;
        }

        Ok(())
    }
});
