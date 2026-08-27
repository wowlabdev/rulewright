#[cfg(test)]
use googletest::prelude::*;

use super::parse;

const MAX_PRECEDING_LINES: usize = 3;

/// Check whether any `keyword` appears in the comment block immediately preceding `line`.
pub(crate) fn has_preceding_comment(lines: &[&str], line: usize, keywords: &[&str]) -> bool {
    if line <= 1 {
        return false;
    }

    let idx = line - 1;
    let start = idx.saturating_sub(MAX_PRECEDING_LINES);

    for i in (start..idx).rev() {
        let Some(raw) = lines.get(i) else {
            break;
        };
        let trimmed = raw.trim();

        if trimmed.is_empty() {
            continue;
        }

        if !parse::is_comment(trimmed) {
            break;
        }

        if keywords.iter().any(|kw| trimmed.contains(kw)) {
            return true;
        }
    }

    false
}

/// Check if `pattern` appears in `line` outside of string literals and comments.
pub(crate) fn contains_outside_strings(line: &str, pattern: &str) -> bool {
    super::scanner::code_only(line).contains(pattern)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gtest]
    #[expect(
        clippy::unnecessary_safety_comment,
        reason = "the adjacent safety comment is the fixture under test"
    )]
    fn preceding_comment_found() -> Result<()> {
        let lines = vec!["// SAFETY: valid pointer", "unsafe { }"];

        verify_true!(has_preceding_comment(&lines, 2, &["SAFETY:"]))?;

        Ok(())
    }

    #[gtest]
    fn preceding_comment_not_found() -> Result<()> {
        let lines = vec!["let x = 1;", "unsafe { }"];

        verify_false!(has_preceding_comment(&lines, 2, &["SAFETY:"]))?;

        Ok(())
    }

    #[gtest]
    #[expect(
        clippy::unnecessary_safety_comment,
        reason = "the blank-separated safety comment is the fixture under test"
    )]
    fn preceding_comment_skips_blanks() -> Result<()> {
        let lines = vec!["// SAFETY: ok", "", "unsafe { }"];

        verify_true!(has_preceding_comment(&lines, 3, &["SAFETY:"]))?;

        Ok(())
    }

    #[gtest]
    fn preceding_comment_multiple_keywords() -> Result<()> {
        let lines = vec!["// LEAK: intentional", "mem::forget(x);"];

        verify_true!(has_preceding_comment(&lines, 2, &["LEAK:", "SAFETY:"]))?;

        Ok(())
    }

    #[gtest]
    #[expect(
        clippy::unnecessary_safety_comment,
        reason = "the interrupted safety comment is the fixture under test"
    )]
    fn preceding_comment_stops_at_code() -> Result<()> {
        let lines = vec!["// SAFETY: ok", "let x = 1;", "unsafe { }"];

        verify_false!(has_preceding_comment(&lines, 3, &["SAFETY:"]))?;

        Ok(())
    }

    #[gtest]
    fn line_one_returns_false() -> Result<()> {
        let lines = vec!["unsafe { }"];

        verify_false!(has_preceding_comment(&lines, 1, &["SAFETY:"]))?;

        Ok(())
    }

    #[gtest]
    fn preceding_comment_ignores_code_with_keyword() -> Result<()> {
        let lines = vec!["let safety = \"SAFETY: nope\";", "unsafe { }"];

        verify_false!(has_preceding_comment(&lines, 2, &["SAFETY:"]))?;

        Ok(())
    }

    #[gtest]
    fn outside_strings_in_code() -> Result<()> {
        verify_true!(contains_outside_strings(
            "static mut X: i32 = 0;",
            "static mut "
        ))?;

        Ok(())
    }

    #[gtest]
    fn outside_strings_in_string_literal() -> Result<()> {
        verify_false!(contains_outside_strings(
            r#"let msg = "static mut is dangerous";"#,
            "static mut "
        ))?;

        Ok(())
    }

    #[gtest]
    fn outside_strings_in_comment() -> Result<()> {
        verify_false!(contains_outside_strings(
            "// static mut X: i32 = 0;",
            "static mut "
        ))?;

        Ok(())
    }

    #[gtest]
    fn outside_strings_mixed() -> Result<()> {
        verify_true!(contains_outside_strings(
            r#"let msg = "hello"; static mut X: i32 = 0;"#,
            "static mut "
        ))?;

        Ok(())
    }

    #[gtest]
    fn outside_strings_char_literal() -> Result<()> {
        verify_false!(contains_outside_strings("let c = '{'; // no match", "{;"))?;

        Ok(())
    }
}
