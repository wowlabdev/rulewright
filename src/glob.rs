#[cfg(test)]
use googletest::prelude::*;

fn compile(pattern: &str) -> Result<globset::GlobMatcher, globset::Error> {
    globset::GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map(|glob| glob.compile_matcher())
}

pub(crate) fn validate(pattern: &str) -> Result<(), String> {
    compile(pattern)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Return `true` if the relative path matches any of the given glob patterns.
pub(crate) fn matches_ignore(rel: &str, patterns: &[impl AsRef<str>]) -> bool {
    patterns
        .iter()
        .any(|pattern| compile(pattern.as_ref()).is_ok_and(|matcher| matcher.is_match(rel)))
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
    fn split_test_module_patterns_match_root_and_nested_files() -> Result<()> {
        let patterns = ["**/*_tests.rs", "**_tests.rs"];

        verify_true!(matches_ignore("parser_tests.rs", &patterns))?;
        verify_true!(matches_ignore(
            "crates/example/src/parser_tests.rs",
            &patterns
        ))?;
        verify_true!(matches_ignore(
            "crates/evade-core/src/planning/prepared_hazards/retained_cache_tests.rs",
            &patterns
        ))?;

        verify_false!(matches_ignore("crates/example/src/parser.rs", &patterns))
    }

    #[gtest]
    fn nested_tests_and_workspace_members_match_recursively() -> Result<()> {
        verify_true!(matches_ignore(
            "crates/example/src/nested/tests.rs",
            &["**/tests.rs"]
        ))?;

        verify_true!(matches_ignore(
            "crates/evade-wasm-tests/src/lib.rs",
            &["crates/evade-wasm-tests/**"]
        ))
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
    fn normalized_paths_have_cross_platform_semantics() -> Result<()> {
        verify_true!(matches_ignore(
            "packages/app/src/main.rs",
            &["packages/app/**"]
        ))?;

        verify_false!(matches_ignore(
            r"packages\app\src\main.rs",
            &["packages/app/**"]
        ))
    }

    #[gtest]
    fn malformed_patterns_are_rejected() -> Result<()> {
        verify_true!(validate("src/[unterminated.rs").is_err())
    }
}
