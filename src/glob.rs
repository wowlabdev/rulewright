#[cfg(test)]
use googletest::prelude::*;

/// Return `true` if the relative path matches any of the given glob patterns.
pub(crate) fn matches_ignore(rel: &str, patterns: &[impl AsRef<str>]) -> bool {
    patterns
        .iter()
        .any(|pat| glob_match::glob_match(pat.as_ref(), rel))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gtest]
    fn literal_path() -> Result<()> {
        verify_true!(matches_ignore(
            "packages/shared/src/parsers/rows.rs",
            &["packages/shared/src/parsers/rows.rs"]
        ))?;

        Ok(())
    }

    #[gtest]
    fn double_star_prefix() -> Result<()> {
        verify_true!(matches_ignore("packages/app/src/main.rs", &["**/main.rs"]))?;
        verify_true!(matches_ignore("main.rs", &["**/main.rs"]))?;

        Ok(())
    }

    #[gtest]
    fn double_star_suffix() -> Result<()> {
        verify_true!(matches_ignore(
            "packages/app/src/commands/check.rs",
            &["packages/app/**"]
        ))?;
        verify_false!(matches_ignore(
            "packages/shared/src/lib.rs",
            &["packages/app/**"]
        ))?;

        Ok(())
    }

    #[gtest]
    fn star_extension() -> Result<()> {
        verify_true!(matches_ignore(
            "packages/service/build.rs",
            &["**/build.rs"]
        ))?;

        Ok(())
    }

    #[gtest]
    fn no_match() -> Result<()> {
        verify_false!(matches_ignore(
            "packages/service/src/lib.rs",
            &["**/main.rs"]
        ))?;

        Ok(())
    }

    #[gtest]
    fn empty_patterns() -> Result<()> {
        let empty: &[&str] = &[];

        verify_false!(matches_ignore("anything.rs", empty))?;

        Ok(())
    }

    #[gtest]
    fn mixed_patterns() -> Result<()> {
        let patterns = &["**/main.rs", "packages/app/**", "**/build.rs"];

        verify_true!(matches_ignore("packages/worker/src/main.rs", patterns))?;
        verify_true!(matches_ignore(
            "packages/app/src/commands/check.rs",
            patterns
        ))?;
        verify_true!(matches_ignore("packages/service/build.rs", patterns))?;
        verify_false!(matches_ignore("packages/service/src/lib.rs", patterns))?;

        Ok(())
    }

    #[gtest]
    fn string_vec_patterns() -> Result<()> {
        let patterns = vec!["**/main.rs".to_string(), "packages/app/**".to_string()];

        verify_true!(matches_ignore("packages/app/src/main.rs", &patterns))?;
        verify_false!(matches_ignore("packages/service/src/lib.rs", &patterns))?;

        Ok(())
    }

    #[gtest]
    fn backslashes_follow_native_path_semantics() -> Result<()> {
        let matches = matches_ignore(r"packages\app\src\main.rs", &["packages/app/**"]);

        if cfg!(windows) {
            verify_true!(matches)
        } else {
            verify_false!(matches)
        }
    }
}
