//! Shared boundaries for source regions opted into by `#rw:` markers.

pub(super) fn is_source_marker(
    line: &str,
    index: usize,
    comment_starts: &[Option<usize>],
    marker: &str,
) -> bool {
    crate::infra::scanner::is_standalone_line_comment(line, index, comment_starts, marker)
}

pub(super) fn ends_region(line: &str) -> bool {
    let trimmed = line.trim();

    trimmed.is_empty() || is_closing_delimiter_line(trimmed)
}

pub(super) fn ends_alignment_region(line: &str) -> bool {
    let trimmed = line.trim();

    ends_region(line) && !trimmed.starts_with("),")
}

fn is_closing_delimiter_line(line: &str) -> bool {
    let code = crate::infra::scanner::code_only(line);
    let code = code.trim();
    let Some(first) = code.chars().next() else {
        return false;
    };

    matches!(first, ')' | ']' | '}')
        && code
            .chars()
            .all(|character| matches!(character, ')' | ']' | '}' | ';' | ',' | '?'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_composite_closing_delimiter_ends_a_region() {
        for line in [
            ")",
            ");",
            "),",
            "));",
            "]",
            "];",
            "],",
            "])",
            "]);",
            "})?;",
            "]); // values",
            "]); /* values */",
        ] {
            assert!(ends_region(line), "{line}");
        }
    }

    #[test]
    fn data_rows_and_expressions_do_not_end_a_region() {
        for line in ["(VALUE, OTHER),", "Value::Item,", "} else {", ".finish();"] {
            assert!(!ends_region(line), "{line}");
        }
    }

    #[test]
    fn a_wrapped_tuple_row_remains_part_of_an_alignment_region() {
        assert!(!ends_alignment_region("    ),"));
        assert!(ends_alignment_region("    ));"));
    }
}
