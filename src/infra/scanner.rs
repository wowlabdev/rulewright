//! Shared code scanner for skipping string literals, char literals, and comments.

#[cfg(test)]
use googletest::prelude::*;
use winnow::token::{any, take_till, take_while};

use super::parse;

/// Return the byte column where each source line's real `//` comment starts.
// #rw(fn: rust_cyclomatic_complexity) lexical state dispatch necessarily branches for each Rust token class
pub(crate) fn line_comment_starts(source: &str) -> Vec<Option<usize>> {
    const TWO_BYTE_TOKEN: usize = 2;

    let bytes = source.as_bytes();
    let mut starts = vec![None];
    let mut line = 0;
    let mut column = 0;
    let mut index = 0;
    let mut state = LexState::Code;

    while let Some(&byte) = bytes.get(index) {
        if byte == b'\n' {
            starts.push(None);
            line += 1;
            column = 0;
            index += 1;

            if matches!(state, LexState::LineComment | LexState::Char { .. }) {
                state = LexState::Code;
            }

            continue;
        }

        let next = bytes.get(index + 1).copied();

        match state {
            LexState::Code => match (byte, next) {
                (b'/', Some(b'/')) => {
                    if let Some(start) = starts.get_mut(line) {
                        *start = Some(column);
                    }

                    state = LexState::LineComment;
                    index += TWO_BYTE_TOKEN;
                    column += TWO_BYTE_TOKEN;
                }

                (b'/', Some(b'*')) => {
                    state = LexState::BlockComment { depth: 1 };
                    index += TWO_BYTE_TOKEN;
                    column += TWO_BYTE_TOKEN;
                }

                (b'"', _) => {
                    state = LexState::String { escaped: false };
                    index += 1;
                    column += 1;
                }

                (b'\'', _) => {
                    state = LexState::Char { escaped: false };
                    index += 1;
                    column += 1;
                }

                (b'r', _) => {
                    let remaining = bytes.get(index..).unwrap_or_default();

                    if let Some((hashes, consumed)) = raw_string_open(remaining) {
                        state = LexState::RawString { hashes };
                        index += consumed;
                        column += consumed;
                    } else {
                        index += 1;
                        column += 1;
                    }
                }
                _ => {
                    index += 1;
                    column += 1;
                }
            },
            LexState::LineComment => {
                index += 1;
                column += 1;
            }
            LexState::BlockComment { depth } => match (byte, next) {
                (b'/', Some(b'*')) => {
                    state = LexState::BlockComment { depth: depth + 1 };
                    index += TWO_BYTE_TOKEN;
                    column += TWO_BYTE_TOKEN;
                }

                (b'*', Some(b'/')) => {
                    state = if depth == 1 {
                        LexState::Code
                    } else {
                        LexState::BlockComment { depth: depth - 1 }
                    };
                    index += TWO_BYTE_TOKEN;
                    column += TWO_BYTE_TOKEN;
                }
                _ => {
                    index += 1;
                    column += 1;
                }
            },
            LexState::String { escaped } | LexState::Char { escaped } => {
                let quote = if matches!(state, LexState::String { .. }) {
                    b'"'
                } else {
                    b'\''
                };

                state = if escaped {
                    if quote == b'"' {
                        LexState::String { escaped: false }
                    } else {
                        LexState::Char { escaped: false }
                    }
                } else if byte == b'\\' {
                    if quote == b'"' {
                        LexState::String { escaped: true }
                    } else {
                        LexState::Char { escaped: true }
                    }
                } else if byte == quote {
                    LexState::Code
                } else {
                    state
                };
                index += 1;
                column += 1;
            }
            LexState::RawString { hashes } => {
                let after_quote = bytes.get(index + 1..).unwrap_or_default();

                if byte == b'"' && raw_string_close(after_quote, hashes) {
                    let consumed = hashes + 1;

                    state = LexState::Code;
                    index += consumed;
                    column += consumed;
                } else {
                    index += 1;
                    column += 1;
                }
            }
        }
    }

    starts
}

pub(crate) fn is_standalone_line_comment(
    line: &str,
    index: usize,
    comment_starts: &[Option<usize>],
    marker: &str,
) -> bool {
    let indentation = line.len() - line.trim_start().len();

    line.trim() == marker && comment_starts.get(index) == Some(&Some(indentation))
}

/// Hide directive-shaped text that is not a standalone source comment.
pub(crate) fn directive_source_lines<'a>(source: &str, lines: &[&'a str]) -> Vec<&'a str> {
    let comment_starts = line_comment_starts(source);

    lines
        .iter()
        .enumerate()
        .map(|(index, &line)| {
            if parse::directive(line).is_none() {
                return line;
            }

            let indentation = line.len() - line.trim_start().len();

            if comment_starts.get(index) == Some(&Some(indentation)) {
                line
            } else {
                ""
            }
        })
        .collect()
}

#[derive(Clone, Copy)]
enum LexState {
    Code,
    LineComment,
    BlockComment { depth: usize },
    String { escaped: bool },
    Char { escaped: bool },
    RawString { hashes: usize },
}

const RAW_DELIMITER_BYTES: usize = 2;

fn raw_string_open(bytes: &[u8]) -> Option<(usize, usize)> {
    let mut hashes = 0;

    while bytes.get(hashes + 1) == Some(&b'#') {
        hashes += 1;
    }

    (bytes.get(hashes + 1) == Some(&b'"')).then_some((hashes, hashes + RAW_DELIMITER_BYTES))
}

fn raw_string_close(bytes: &[u8], hashes: usize) -> bool {
    bytes
        .get(..hashes)
        .is_some_and(|suffix| suffix.iter().all(|byte| *byte == b'#'))
}

/// Skip past a `"..."` string literal, assuming the opening `"` is already consumed.
pub(crate) fn skip_string(input: &mut &str) {
    loop {
        let _ = parse::try_parse(input, take_till(0.., ['\\', '"']));

        match parse::try_parse(input, any) {
            Some('\\') => {
                let _ = parse::try_parse(input, any);
            }
            _ => return,
        }
    }
}

/// Skip past a raw string literal with `hashes` closing `#`, opening `"` already consumed.
pub(crate) fn skip_raw_string(input: &mut &str, hashes: usize) {
    loop {
        let _ = parse::try_parse(input, take_till(0.., '"'));

        if parse::try_parse(input, any).is_none() {
            return;
        }

        let mut matched = 0;

        while matched < hashes && parse::try_parse(input, '#').is_some() {
            matched += 1;
        }

        if matched == hashes {
            return;
        }
    }
}

/// Skip a character literal, assuming the opening `'` is already consumed.
pub(crate) fn skip_char_literal(input: &mut &str) {
    let _ = parse::try_parse(input, '\\');
    let _ = parse::try_parse(input, any);
    let _ = parse::try_parse(input, '\'');
}

/// Consume a raw string prefix and body after an `r`, `true` if one was consumed.
pub(crate) fn try_skip_raw_string(input: &mut &str) -> bool {
    let hashes = parse::try_parse(input, take_while(0.., '#')).map_or(0, |s: &str| s.len());

    if parse::try_parse(input, '"').is_some() {
        skip_raw_string(input, hashes);

        true
    } else {
        false
    }
}

/// Return a copy of `line` with literals and comments stripped out.
pub(crate) fn code_only(line: &str) -> String {
    let mut code = String::with_capacity(line.len());
    let mut input = line;

    while let Some(ch) = parse::try_parse(&mut input, any) {
        match ch {
            '/' if parse::matches(input, '/') => break,
            '/' if parse::try_parse(&mut input, '*').is_some() => {
                skip_block_comment(&mut input);
            }
            '"' => skip_string(&mut input),
            'r' if parse::matches(input, '#') || parse::matches(input, '"') => {
                if !try_skip_raw_string(&mut input) {
                    code.push(ch);
                }
            }
            '\'' => skip_char_literal(&mut input),
            _ => code.push(ch),
        }
    }

    code
}

fn skip_block_comment(input: &mut &str) {
    let mut depth = 1;

    while let Some(ch) = parse::try_parse(input, any) {
        match ch {
            '/' if parse::try_parse(input, '*').is_some() => depth += 1,
            '*' if parse::try_parse(input, '/').is_some() => {
                depth -= 1;

                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
    }
}

/// Return the byte offsets of every occurrence of `target` in `line`, skipping string and char literals.
pub(crate) fn char_positions(line: &str, target: char) -> Vec<usize> {
    let mut out = Vec::new();
    let mut input = line;
    let mut pos = 0;

    while let Some(ch) = parse::try_parse(&mut input, any) {
        match ch {
            '/' if parse::matches(input, '/') => break,
            '"' => {
                skip_string(&mut input);
                pos = line.len() - input.len();
                continue;
            }

            'r' if parse::matches(input, '#') || parse::matches(input, '"') => {
                if try_skip_raw_string(&mut input) {
                    pos = line.len() - input.len();
                    continue;
                }

                if ch == target {
                    out.push(pos);
                }

                pos += ch.len_utf8();
                continue;
            }

            '\'' => {
                skip_char_literal(&mut input);
                pos = line.len() - input.len();
                continue;
            }
            c if c == target => {
                out.push(pos);
            }
            _ => {}
        }

        pos += ch.len_utf8();
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gtest]
    fn code_only_strips_strings() -> Result<()> {
        verify_eq!(code_only(r#"let x = "hello"; y"#), "let x = ; y")?;

        Ok(())
    }

    #[gtest]
    fn code_only_strips_comments() -> Result<()> {
        verify_eq!(code_only("let x = 1; // comment"), "let x = 1; ")?;
        verify_eq!(
            code_only("let /* outer /* inner */ done */ x = 1;"),
            "let  x = 1;"
        )?;

        Ok(())
    }

    #[gtest]
    fn code_only_strips_char_literals() -> Result<()> {
        verify_eq!(code_only("let c = '{'; rest"), "let c = ; rest")?;

        Ok(())
    }

    #[gtest]
    fn code_only_strips_raw_strings() -> Result<()> {
        let input = "let s = r\"raw\"; rest";

        verify_eq!(code_only(input), "let s = ; rest")?;

        Ok(())
    }

    #[gtest]
    fn char_positions_basic() -> Result<()> {
        verify_eq!(char_positions("a, b, c", ','), vec![1, 4])?;

        Ok(())
    }

    #[gtest]
    fn char_positions_skips_strings() -> Result<()> {
        let input = "call(a, \"hello, world\", b)";

        verify_eq!(char_positions(input, ','), vec![6, 22])?;

        Ok(())
    }

    #[gtest]
    fn char_positions_skips_char_literals() -> Result<()> {
        let input = "let c = '\\'' ; x, y";
        let positions = char_positions(input, ',');

        verify_eq!(positions, vec![16])?;

        Ok(())
    }

    #[gtest]
    fn char_positions_skips_comments() -> Result<()> {
        verify_eq!(char_positions("a, b // c, d", ','), vec![1])?;

        Ok(())
    }

    #[gtest]
    fn comment_lines_exclude_multiline_string_lookalikes() -> Result<()> {
        let source =
            "let fixture = r#\"\n// #rw(rust_panic) not source\n\"#;\n// #rw(rust_dbg) source\n";
        let starts = line_comment_starts(source);

        verify_eq!(starts, vec![None, None, None, Some(0), None])?;

        Ok(())
    }

    #[gtest]
    fn comment_lines_exclude_nested_block_comment_lookalikes() -> Result<()> {
        let source = "/* outer\n/* nested */\n// #rw(rust_panic) not source\n*/\n    // #rw(rust_dbg) source";
        let starts = line_comment_starts(source);

        verify_eq!(starts, vec![None, None, None, None, Some(4)])?;

        Ok(())
    }

    #[gtest]
    fn directive_lines_exclude_non_comment_lookalikes() -> Result<()> {
        let source = concat!(
            "let fixture = r#\"\n",
            "// #rw(file: rust_panic) raw string\n",
            "\"#;\n",
            "/*\n",
            "// #rw(file: rust_dbg) block comment\n",
            "*/\n",
            "    // #rw(rust_panic) real directive\n",
            "panic!(\"boom\");",
        );
        let lines: Vec<_> = source.lines().collect();
        let visible = directive_source_lines(source, &lines);

        verify_eq!(visible[1], "")?;
        verify_eq!(visible[4], "")?;
        verify_eq!(visible[6], "    // #rw(rust_panic) real directive")?;

        Ok(())
    }
}
