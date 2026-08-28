use ra_ap_syntax::{AstNode, Edition, SourceFile, ast};

use crate::{FileCtx, Fix, Violation};

use super::{
    MIN_ALIGNED_ROWS, Sep, collect_block, comma_positions, comment_position, detect_separator,
    is_source_marker,
};

pub(super) fn fix_aligned(ctx: &FileCtx<'_>, violation: &Violation) -> Option<Fix> {
    let (start, block) = containing_block(ctx, violation.line)?;
    let texts: Vec<&str> = block.iter().map(|(_, line)| *line).collect();
    let end = block.last()?.0;
    let mut replacement: Vec<String> = ctx
        .lines
        .get(start..=end)?
        .iter()
        .map(|line| (*line).to_owned())
        .collect();

    if block.iter().any(|(_, line)| line.trim() == "(") {
        let mut collapsed = collapse_tuple_rows(&block)?;
        let snapshot = collapsed.clone();
        let collapsed_block: Vec<(usize, &str)> = snapshot
            .iter()
            .enumerate()
            .map(|(index, line)| (index, line.as_str()))
            .collect();

        align_commas(&collapsed_block, 0, &mut collapsed)?;
        align_comments(&collapsed_block, 0, &mut collapsed);

        if !collapsed
            .iter()
            .filter(|line| is_tuple_row(line))
            .all(|line| {
                parse_simple_tuple(line).is_some_and(|fields| fields.len() >= MIN_ALIGNED_ROWS)
            })
        {
            return None;
        }

        return Some(Fix::replace_lines(
            start + 1,
            block.last()?.0 + 1,
            collapsed.join("\n"),
        ));
    }

    match detect_separator(&texts)? {
        Sep::Arrow => align_arrows(&block, start, &mut replacement),
        Sep::Comma => align_commas(&block, start, &mut replacement)?,
    }

    align_comments(&block, start, &mut replacement);

    Some(Fix::replace_lines(
        start + 1,
        block.last()?.0 + 1,
        replacement.join("\n"),
    ))
}

fn collapse_tuple_rows(block: &[(usize, &str)]) -> Option<Vec<String>> {
    if !block
        .windows(2)
        .all(|pair| pair[1].0 == pair[0].0.saturating_add(1))
    {
        return None;
    }

    let mut collapsed = Vec::new();
    let mut cursor = 0;
    let mut changed = false;

    while cursor < block.len() {
        let &(_, line) = block.get(cursor)?;

        if line.trim() != "(" {
            collapsed.push(line.to_owned());
            cursor += 1;
            continue;
        }

        let end = block
            .iter()
            .enumerate()
            .skip(cursor + 1)
            .find(|(_, (_, candidate))| candidate.trim() == "),")
            .map(|(index, _)| index)?;
        let row = block
            .get(cursor..=end)?
            .iter()
            .map(|(_, field)| *field)
            .collect::<Vec<_>>()
            .join("\n");
        let fields = parse_simple_tuple(&row)?;

        if fields.len() < MIN_ALIGNED_ROWS {
            return None;
        }

        let indentation = line.get(..line.len() - line.trim_start().len())?;

        collapsed.push(format!("{indentation}({}),", fields.join(", ")));
        cursor = end + 1;
        changed = true;
    }

    changed.then_some(collapsed)
}

fn is_tuple_row(line: &str) -> bool {
    let trimmed = line.trim();

    trimmed.starts_with('(') && trimmed.ends_with("),")
}

fn parse_simple_tuple(source: &str) -> Option<Vec<String>> {
    let expression = source.trim().strip_suffix(',')?;
    let wrapper = format!("fn __rulewright_aligned() {{ let _ = {expression}; }}");
    let parse = SourceFile::parse(&wrapper, Edition::Edition2024);

    if !parse.errors().is_empty() {
        return None;
    }

    let root = parse.tree();
    let tuple = root.syntax().descendants().find_map(ast::TupleExpr::cast)?;
    let fields: Vec<ast::Expr> = tuple.fields().collect();

    if fields.iter().any(|field| {
        !matches!(field, ast::Expr::Literal(_) | ast::Expr::PathExpr(_))
            || field
                .syntax()
                .descendants_with_tokens()
                .filter_map(ra_ap_syntax::NodeOrToken::into_token)
                .any(|token| token.kind() == ra_ap_syntax::SyntaxKind::COMMENT)
    }) {
        return None;
    }

    Some(
        fields
            .into_iter()
            .map(|field| field.syntax().text().to_string())
            .collect(),
    )
}

fn containing_block<'a>(
    ctx: &'a FileCtx<'_>,
    violation_line: usize,
) -> Option<(usize, Vec<(usize, &'a str)>)> {
    let comment_starts = crate::infra::scanner::line_comment_starts(ctx.contents);

    ctx.lines.iter().enumerate().find_map(|(index, line)| {
        if !is_source_marker(line, index, &comment_starts) {
            return None;
        }

        let block = collect_block(ctx.lines, index + 1);

        block
            .iter()
            .any(|(line_index, _)| *line_index + 1 == violation_line)
            .then_some((index + 1, block))
    })
}

fn align_arrows(block: &[(usize, &str)], start: usize, replacement: &mut [String]) {
    let target = block
        .iter()
        .filter_map(|(_, line)| line.find("=>"))
        .max()
        .unwrap_or_default();

    for (index, line) in block {
        let Some(position) = line.find("=>") else {
            continue;
        };

        replacement[*index - start].insert_str(position, &" ".repeat(target - position));
    }
}

// #rw(fn: rust_collection_new_in_loop) each row needs an independently rebuilt aligned representation
fn align_commas(block: &[(usize, &str)], start: usize, replacement: &mut [String]) -> Option<()> {
    let segments: Vec<Vec<&str>> = block
        .iter()
        .map(|(_, line)| split_at_commas(line))
        .collect();
    let count = segments.first()?.len();

    if count < MIN_ALIGNED_ROWS || segments.iter().any(|row| row.len() != count) {
        return None;
    }

    let widths: Vec<usize> = (0..count - 1)
        .map(|column| {
            segments
                .iter()
                .map(|row| {
                    if column == 0 {
                        row[column].trim_end().len()
                    } else {
                        row[column].trim().len()
                    }
                })
                .max()
                .unwrap_or_default()
        })
        .collect();

    for ((index, _), row) in block.iter().zip(&segments) {
        let mut line = String::new();

        for (column, segment) in row.iter().enumerate() {
            let segment = if column == 0 {
                segment.trim_end()
            } else {
                segment.trim()
            };

            if column > 0 && !segment.is_empty() {
                let previous = if column == 1 {
                    row[column - 1].trim_end().len()
                } else {
                    row[column - 1].trim().len()
                };

                line.push_str(&" ".repeat(widths[column - 1] - previous + 1));
            }

            line.push_str(segment);

            if column < row.len() - 1 {
                line.push(',');
            }
        }

        replacement[*index - start] = line;
    }

    Some(())
}

fn align_comments(block: &[(usize, &str)], start: usize, replacement: &mut [String]) {
    let commented: Vec<(usize, usize)> = block
        .iter()
        .filter_map(|(index, _)| {
            let row = replacement.get(*index - start)?;

            comment_position(row).map(|position| (*index, position))
        })
        .collect();

    if commented.len() < MIN_ALIGNED_ROWS {
        return;
    }

    let target = commented
        .iter()
        .filter_map(|(index, position)| {
            replacement
                .get(*index - start)
                .and_then(|row| row.get(..*position))
                .map(str::trim_end)
                .map(str::len)
        })
        .max()
        .unwrap_or_default()
        + 1;

    for (index, position) in commented {
        let Some(row) = replacement.get_mut(index - start) else {
            continue;
        };
        let Some(comment) = row.get(position..).map(str::to_owned) else {
            continue;
        };

        row.truncate(position);
        let code_len = row.trim_end().len();

        row.truncate(code_len);
        row.push_str(&" ".repeat(target - code_len));
        row.push_str(&comment);
    }
}

fn split_at_commas(line: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut start = 0;

    for position in comma_positions(line) {
        segments.push(&line[start..position]);
        start = position + 1;
    }

    segments.push(&line[start..]);

    segments
}
