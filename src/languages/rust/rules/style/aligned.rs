#[cfg(test)]
use googletest::prelude::*;
use std::{cmp::Reverse, collections::BTreeMap};

use winnow::token::take_until;

use crate::{Example, FileCtx, Violation, infra::parse, violation};

use fix::fix_aligned;

mod fix;

const MARKER: &str = "// #rw:aligned";
const MIN_ALIGNED_ROWS: usize = 2;
const MAJORITY_DIVISOR: usize = 2;

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "arrow aligned",
        code: "// #rw:aligned\nParser    => \"a\";\nWriter    => \"b\";\nFiles     => \"c\";",
        pass: true,
    },
    Example {
        label: "arrow misaligned",
        code: "// #rw:aligned\nParser    => \"a\";\nWriter => \"b\";\nFiles     => \"c\";",
        pass: false,
    },
    Example {
        label: "comma aligned",
        code: "// #rw:aligned\ncall(a,  \"x\",  TypeA);\ncall(b,  \"y\",  TypeB);",
        pass: true,
    },
    Example {
        label: "no marker",
        code: "Parser    => \"a\";\nWriter => \"b\";",
        pass: true,
    },
    Example {
        label: "comma misaligned",
        code: "// #rw:aligned\ncall(a,  \"x\",  TypeA);\ncall(long_name, \"y\", TypeB);",
        pass: false,
    },
    Example {
        label: "trailing comments aligned",
        code: "// #rw:aligned\n(A, 1), // first\n(B, 2), // second",
        pass: true,
    },
    Example {
        label: "trailing comments misaligned",
        code: "// #rw:aligned\n(A, 1),      // first\n(B, 2), // second",
        pass: false,
    },
    Example {
        label: "wrapped tuple row",
        code: "// #rw:aligned\n(\n    SHORT,\n    \"first\",\n),\n(LONG_NAME, \"second\"),",
        pass: false,
    },
];

crate::full_line_rule!(
    aligned,
    "Enforce column alignment in regions marked with `// #rw:aligned`.",
    "Consistent column alignment in marked regions makes tabular data and match arms easier to scan.",
    Low,
    fix_aligned,
);

fn check_aligned(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let mut out = Vec::new();
    let mut i = 0;
    let comment_starts = crate::infra::scanner::line_comment_starts(ctx.contents);

    while i < ctx.lines.len() {
        if ctx
            .lines
            .get(i)
            .is_some_and(|line| is_source_marker(line, i, &comment_starts))
        {
            let marker_line = i + 1;
            let block = collect_block(ctx.lines, i + 1);

            if block.len() >= MIN_ALIGNED_ROWS {
                for &(index, line) in &block {
                    if line.trim() == "(" {
                        out.push(violation(
                            ctx.rel,
                            index + 1,
                            format!(
                                "tuple row must stay on one line (aligned block at line {marker_line})"
                            ),
                        ));
                    }
                }

                let texts: Vec<&str> = block.iter().map(|&(_, s)| s).collect();

                match detect_separator(&texts) {
                    Some(Sep::Arrow) => {
                        check_arrow(ctx.rel, &block, marker_line, &mut out);
                    }

                    Some(Sep::Comma) => {
                        check_comma(ctx.rel, &block, marker_line, &mut out);
                    }

                    None => {}
                }

                check_comments(ctx.rel, &block, marker_line, &mut out);
            }

            i = block.last().map_or(i + 1, |&(idx, _)| idx + 1);
        } else {
            i += 1;
        }
    }

    out
}

fn is_marker(line: &str) -> bool {
    line.trim() == MARKER
}

fn is_source_marker(line: &str, index: usize, comment_starts: &[Option<usize>]) -> bool {
    super::marked_region::is_source_marker(line, index, comment_starts, MARKER)
}

fn collect_block<'a>(lines: &[&'a str], start: usize) -> Vec<(usize, &'a str)> {
    let mut block = Vec::new();

    for (i, &line) in lines.iter().enumerate().skip(start) {
        let trimmed = line.trim();

        if super::marked_region::ends_alignment_region(line) || is_marker(line) {
            break;
        }

        if parse::is_comment(trimmed) {
            continue;
        }

        block.push((i, line));
    }

    block
}

enum Sep {
    Arrow,
    Comma,
}

fn detect_separator(block: &[&str]) -> Option<Sep> {
    let arrows = block.iter().filter(|l| l.contains("=>")).count();

    if arrows > block.len() / MAJORITY_DIVISOR {
        return Some(Sep::Arrow);
    }

    let commas = block.iter().filter(|l| l.contains(',')).count();

    if commas > block.len() / MAJORITY_DIVISOR {
        return Some(Sep::Comma);
    }

    None
}

fn check_arrow(rel: &str, block: &[(usize, &str)], marker_line: usize, out: &mut Vec<Violation>) {
    let positions: Vec<(usize, usize)> = block
        .iter()
        .filter_map(|&(idx, line)| {
            let mut input = line;
            let before: &str = parse::try_parse(&mut input, take_until(0.., "=>"))?;

            Some((idx, before.len()))
        })
        .collect();

    if positions.len() < MIN_ALIGNED_ROWS {
        return;
    }

    let expected = majority(positions.iter().map(|&(_, p)| p));

    for &(idx, pos) in &positions {
        if pos != expected {
            out.push(violation(
                rel,
                idx + 1,
                format!(
                    "`=>` at column {pos}, expected {expected} \
                     (aligned block at line {marker_line})"
                ),
            ));
        }
    }
}

fn check_comma(rel: &str, block: &[(usize, &str)], marker_line: usize, out: &mut Vec<Violation>) {
    let mut all: Vec<(usize, Vec<usize>)> = Vec::new();

    for &(idx, line) in block {
        let positions = field_positions(line);

        if !positions.is_empty() {
            all.push((idx, positions));
        }
    }

    if all.len() < MIN_ALIGNED_ROWS {
        return;
    }

    let expected_count = majority(all.iter().map(|(_, p)| p.len()));

    for &(idx, ref positions) in &all {
        if positions.len() != expected_count {
            out.push(violation(
                rel,
                idx + 1,
                format!(
                    "{} aligned fields, expected {expected_count} \
                     (aligned block at line {marker_line})",
                    positions.len()
                ),
            ));
        }
    }

    let matching: Vec<&(usize, Vec<usize>)> = all
        .iter()
        .filter(|(_, p)| p.len() == expected_count)
        .collect();

    if matching.len() < MIN_ALIGNED_ROWS {
        return;
    }

    for col in 0..expected_count {
        let expected_pos = majority(
            matching
                .iter()
                .filter_map(|(_, positions)| positions.get(col).copied()),
        );

        for &&(idx, ref positions) in &matching {
            let Some(actual) = positions.get(col).copied() else {
                continue;
            };

            if actual != expected_pos {
                out.push(violation(
                    rel,
                    idx + 1,
                    format!(
                        "field {} starts at column {}, expected {expected_pos} \
                         (aligned block at line {marker_line})",
                        col + 1,
                        actual
                    ),
                ));
                break;
            }
        }
    }
}

fn comma_positions(line: &str) -> Vec<usize> {
    crate::infra::scanner::char_positions(line, ',')
}

fn field_positions(line: &str) -> Vec<usize> {
    comma_positions(line)
        .into_iter()
        .filter_map(|comma| {
            let remaining = line.get(comma + 1..)?;
            let leading = remaining.len() - remaining.trim_start().len();
            let field = remaining.trim_start();

            (!field.is_empty() && !field.starts_with("//")).then_some(comma + 1 + leading)
        })
        .collect()
}

fn comment_position(line: &str) -> Option<usize> {
    crate::infra::scanner::line_comment_starts(line)
        .into_iter()
        .next()
        .flatten()
}

fn check_comments(
    rel: &str,
    block: &[(usize, &str)],
    marker_line: usize,
    out: &mut Vec<Violation>,
) {
    let positions: Vec<(usize, usize)> = block
        .iter()
        .filter_map(|&(index, line)| comment_position(line).map(|position| (index, position)))
        .collect();

    if positions.len() < MIN_ALIGNED_ROWS {
        return;
    }

    let expected = majority(positions.iter().map(|&(_, position)| position));

    if let Some(&(index, actual)) = positions
        .iter()
        .find(|&&(_, position)| position != expected)
    {
        out.push(violation(
            rel,
            index + 1,
            format!(
                "trailing comment at column {actual}, expected {expected} \
                 (aligned block at line {marker_line})"
            ),
        ));
    }
}

fn majority<T>(iter: impl Iterator<Item = T>) -> T
where
    T: Ord + Copy,
{
    let mut counts: BTreeMap<T, usize> = BTreeMap::default();

    for v in iter {
        *counts.entry(v).or_default() += 1;
    }

    counts
        .into_iter()
        .max_by_key(|&(value, count)| (count, Reverse(value)))
        .expect("majority called on empty iterator")
        .0
}

#[cfg(test)]
mod test_registration;
