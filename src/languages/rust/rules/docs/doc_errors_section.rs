use ra_ap_syntax::ast::{self, HasName, HasVisibility, VisibilityKind};

use super::support::{doc_lines, has_heading, is_item_or_impl_fn};
use crate::{AstCtx, Example, Violation};

/// Pass/fail cases for `example_tests!`.
#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "documented Result fn without Errors section",
        code: "/// Parses input.\npub fn f() -> Result<(), Error> { Ok(()) }",
        pass: false,
    },
    Example {
        label: "documented Result fn with Errors section",
        code: "/// Parses input.\n///\n/// # Errors\n/// Fails on malformed input.\npub fn f() -> Result<(), Error> { Ok(()) }",
        pass: true,
    },
    Example {
        label: "undocumented Result fn is pub_api_docs territory",
        code: "pub fn f() -> Result<(), Error> { Ok(()) }",
        pass: true,
    },
    Example {
        label: "documented fn without Result",
        code: "/// Adds one.\npub fn f(x: u32) -> u32 { x }",
        pass: true,
    },
    Example {
        label: "private documented Result fn",
        code: "/// Parses input.\nfn f() -> Result<(), Error> { Ok(()) }",
        pass: true,
    },
    Example {
        label: "Result type alias counts",
        code: "/// Reads bytes.\npub fn f() -> io::Result<()> { Ok(()) }",
        pass: false,
    },
    Example {
        label: "impl fn without Errors section",
        code: "struct S;\nimpl S {\n    /// Parses input.\n    pub fn f(&self) -> Result<(), Error> { Ok(()) }\n}",
        pass: false,
    },
    Example {
        label: "impl fn with Errors section",
        code: "struct S;\nimpl S {\n    /// Parses input.\n    ///\n    /// # Errors\n    /// Fails on malformed input.\n    pub fn f(&self) -> Result<(), Error> { Ok(()) }\n}",
        pass: true,
    },
    Example {
        label: "doc(hidden) has no doc text",
        code: "#[doc(hidden)]\npub fn f() -> Result<(), Error> { Ok(()) }",
        pass: true,
    },
    Example {
        label: "test module fn",
        code: "#[cfg(test)]\nmod tests {\n    /// Parses input.\n    pub fn f() -> Result<(), Error> { Ok(()) }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    doc_errors_section,
    "Require a `# Errors` section on documented pub fns returning `Result`.",
    "Callers of publishable packages need failure conditions listed; canonical docs put them under `# Errors`. Packages that explicitly set `publish = false` are treated as internal workspace implementation and skipped.",
    Medium,
);

fn check_doc_errors_section(ctx: &AstCtx<'_>) -> Vec<Violation> {
    if ctx.file.is_explicitly_non_publishable() {
        return Vec::new();
    }

    ctx.nodes::<ast::Fn>()
        .filter(is_item_or_impl_fn)
        .filter(|function| !ctx.is_in_test(function) && is_fully_public(function))
        .filter(returns_result)
        .filter_map(|function| {
            let docs = doc_lines(&function);

            if docs.is_empty() || has_heading(&docs, "Errors") {
                return None;
            }

            let name = function.name()?;

            Some(ctx.violation(
                &name,
                format!(
                    "documented `{}` returns `Result` but its docs have no `# Errors` section",
                    name.text()
                ),
            ))
        })
        .collect()
}

fn is_fully_public(function: &ast::Fn) -> bool {
    function
        .visibility()
        .is_some_and(|visibility| matches!(visibility.kind(), VisibilityKind::Pub))
}

fn returns_result(function: &ast::Fn) -> bool {
    let Some(ast::Type::PathType(path_type)) = function.ret_type().and_then(|ret| ret.ty()) else {
        return false;
    };

    let name = path_type
        .path()
        .and_then(|path| path.segment())
        .and_then(|segment| segment.name_ref());

    name.is_some_and(|name| name.text().ends_with("Result"))
}

crate::rulewright_ast_test!(check_doc_errors_section, {
    crate::example_tests!(EXAMPLES, check_doc_errors_section);

    #[test]
    fn package_publishability_controls_downstream_error_docs() {
        let source = "/// Parses input.\npub fn parse() -> Result<(), Error> { Ok(()) }";

        assert_eq!(
            crate::test_support::check_source_ast_at(
                "fixture.rs",
                source,
                check_doc_errors_section,
            )
            .len(),
            1
        );
        assert!(
            crate::test_support::check_source_ast_publishability(
                source,
                false,
                check_doc_errors_section,
            )
            .is_empty()
        );
        assert_eq!(
            crate::test_support::check_source_ast_publishability(
                source,
                true,
                check_doc_errors_section,
            )
            .len(),
            1
        );
    }
});
