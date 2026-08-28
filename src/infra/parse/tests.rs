// #rw(file: rust_duplicate_strings) The linter harness intentionally repeats source fixtures across independent rule scenarios.

use googletest::prelude::*;

use super::*;

#[gtest]
fn comment_content_doc() -> Result<()> {
    verify_eq!(comment_content("/// hello"), Some(" hello"))?;

    Ok(())
}

#[gtest]
fn comment_content_inner_doc() -> Result<()> {
    verify_eq!(comment_content("//! world"), Some(" world"))?;

    Ok(())
}

#[gtest]
fn comment_content_regular() -> Result<()> {
    verify_eq!(comment_content("// plain"), Some(" plain"))?;

    Ok(())
}

#[gtest]
fn comment_content_not_comment() -> Result<()> {
    verify_eq!(comment_content("let x = 1;"), None)?;

    Ok(())
}

#[gtest]
fn comment_content_untrimmed_is_none() -> Result<()> {
    verify_eq!(comment_content("  /// indented"), None)?;

    Ok(())
}

#[gtest]
fn comment_content_trimmed_input() -> Result<()> {
    verify_eq!(comment_content("/// indented"), Some(" indented"))?;

    Ok(())
}

#[gtest]
fn doc_comment_content_doc() -> Result<()> {
    verify_eq!(doc_comment_content("/// hello"), Some(" hello"))?;

    Ok(())
}

#[gtest]
fn doc_comment_content_regular_is_none() -> Result<()> {
    verify_eq!(doc_comment_content("// plain"), None)?;

    Ok(())
}

#[gtest]
fn balanced_simple() -> Result<()> {
    let (b, i, a) = balanced_extract("x = dbg!(foo); y", "dbg!(").or_fail()?;

    verify_eq!(b, "x = ")?;
    verify_eq!(i, "foo")?;
    verify_eq!(a, "; y")?;

    Ok(())
}

#[gtest]
fn balanced_string_with_parens() -> Result<()> {
    let input = "dbg!(\")\")";
    let (b, i, a) = balanced_extract(input, "dbg!(").or_fail()?;

    verify_eq!(b, "")?;
    verify_eq!(i, "\")\"")?;
    verify_eq!(a, "")?;

    Ok(())
}

#[gtest]
fn balanced_nested() -> Result<()> {
    let (b, i, a) = balanced_extract("dbg!(foo(bar(1)))", "dbg!(").or_fail()?;

    verify_eq!(b, "")?;
    verify_eq!(i, "foo(bar(1))")?;
    verify_eq!(a, "")?;

    Ok(())
}

#[gtest]
fn balanced_not_found() -> Result<()> {
    verify_true!(balanced_extract("let x = 1;", "dbg!(").is_none())?;

    Ok(())
}

#[gtest]
fn balanced_unmatched() -> Result<()> {
    verify_true!(balanced_extract("dbg!(foo(bar)", "dbg!(").is_none())?;

    Ok(())
}

#[gtest]
fn directive_next_line() -> Result<()> {
    let d = directive("// #rw(rust_panic) startup invariant").or_fail()?;
    let DirectiveResult::Valid(d) = d else {
        panic!("expected valid")
    };

    verify_eq!(d.scope, Scope::NextLine)?;
    verify_eq!(d.rules, vec!["rust_panic"])?;
    verify_false!(d.wildcard)?;
    verify_eq!(d.reason, "startup invariant")?;
    verify_true!(d.values.is_empty())?;

    Ok(())
}

#[gtest]
fn directive_file_scope() -> Result<()> {
    let d = directive("// #rw(file: rust_commented_code) generated file").or_fail()?;
    let DirectiveResult::Valid(d) = d else {
        panic!("expected valid")
    };

    verify_eq!(d.scope, Scope::File)?;
    verify_eq!(d.rules, vec!["rust_commented_code"])?;

    Ok(())
}

#[gtest]
fn directive_file_budget() -> Result<()> {
    let d = directive(
        "// #rw(file: rust_too_many_lines_in_file = 1450) cohesive generated-style table",
    )
    .or_fail()?;
    let DirectiveResult::Valid(d) = d else {
        panic!("expected valid")
    };

    verify_eq!(d.rules, vec!["rust_too_many_lines_in_file"])?;
    verify_eq!(d.values.get("rust_too_many_lines_in_file"), Some(&1450))?;

    Ok(())
}

#[gtest]
fn directive_budget_requires_file_scope() -> Result<()> {
    let d = directive("// #rw(rust_panic = 10) invalid budget").or_fail()?;

    verify_true!(matches!(d, DirectiveResult::Malformed(_)))?;

    Ok(())
}

#[gtest]
fn directive_block_scope() -> Result<()> {
    let d = directive("// #rw(block: rust_wildcard_imports) barrel re-exports").or_fail()?;
    let DirectiveResult::Valid(d) = d else {
        panic!("expected valid")
    };

    verify_eq!(d.scope, Scope::Block)?;

    Ok(())
}

#[gtest]
// #rw(fn: rust_duplicate_strings) This linter scenario intentionally repeats source fixtures used to verify independent paths.
fn directive_fn_scope() -> Result<()> {
    // #rw(rust_duplicate_strings) This linter fixture intentionally repeats source text across independent rule-harness scenarios.
    let d = directive("// #rw(fn: rust_panic) error reporting function").or_fail()?;
    let DirectiveResult::Valid(d) = d else {
        panic!("expected valid")
    };

    verify_eq!(d.scope, Scope::Fn)?;

    Ok(())
}

#[gtest]
fn directive_multi_rule() -> Result<()> {
    let d = directive("// #rw(rust_panic, rust_dbg) error path").or_fail()?;
    let DirectiveResult::Valid(d) = d else {
        panic!("expected valid")
    };

    verify_eq!(d.rules, vec!["rust_panic", "rust_dbg"])?;

    Ok(())
}

#[gtest]
fn directive_wildcard() -> Result<()> {
    let d = directive("// #rw(*) generated code").or_fail()?;
    let DirectiveResult::Valid(d) = d else {
        panic!("expected valid")
    };

    verify_true!(d.wildcard)?;
    verify_true!(d.rules.is_empty())?;

    Ok(())
}

#[gtest]
fn directive_file_wildcard() -> Result<()> {
    let d = directive("// #rw(file: *) fully generated").or_fail()?;
    let DirectiveResult::Valid(d) = d else {
        panic!("expected valid")
    };

    verify_eq!(d.scope, Scope::File)?;
    verify_true!(d.wildcard)?;

    Ok(())
}

#[gtest]
fn directive_missing_reason() -> Result<()> {
    let d = directive("// #rw(rust_panic)").or_fail()?;

    verify_true!(matches!(d, DirectiveResult::MissingReason(Scope::NextLine)))?;

    Ok(())
}

#[gtest]
fn directive_empty_parens() -> Result<()> {
    let d = directive("// #rw()").or_fail()?;

    verify_true!(matches!(d, DirectiveResult::Malformed(_)))?;

    Ok(())
}

#[gtest]
fn directive_missing_close() -> Result<()> {
    let d = directive("// #rw(rust_panic reason").or_fail()?;

    verify_true!(matches!(d, DirectiveResult::Malformed(_)))?;

    Ok(())
}

#[gtest]
fn directive_not_directive() -> Result<()> {
    verify_true!(directive("let x = 1;").is_none())?;
    verify_true!(directive("// regular comment").is_none())?;

    Ok(())
}

#[gtest]
fn directive_rejects_wildcard_mixed_with_named_target() -> Result<()> {
    let result = directive("// #rw(*, rust_panic) ambiguous wildcard").or_fail()?;

    verify_true!(
        matches!(result, DirectiveResult::Malformed(message) if message.contains("only target"))
    )?;

    Ok(())
}

#[gtest]
fn rewrite_directive_rules_preserves_scope_reason_and_indentation() -> Result<()> {
    let rewritten = rewrite_directive_rules(
        "    // #rw(block: rust_dbg, rust_panic) exact reason",
        &Scope::Block,
        &["rust_dbg"],
    );

    verify_eq!(
        rewritten.as_deref(),
        Some("    // #rw(block: rust_dbg) exact reason")
    )?;

    Ok(())
}

#[gtest]
fn is_allow_attr_item() -> Result<()> {
    verify_true!(is_allow_attr("#[allow(dead_code)]"))?;

    Ok(())
}

#[gtest]
fn is_allow_attr_module() -> Result<()> {
    verify_true!(is_allow_attr("#![allow(clippy::too_many_arguments)]"))?;

    Ok(())
}

#[gtest]
fn is_allow_attr_other() -> Result<()> {
    verify_false!(is_allow_attr("#[derive(Debug)]"))?;

    Ok(())
}

#[gtest]
fn char_after_slashes_space() -> Result<()> {
    verify_eq!(char_after_slashes("// comment"), Some(' '))?;

    Ok(())
}

#[gtest]
fn char_after_slashes_letter() -> Result<()> {
    verify_eq!(char_after_slashes("//comment"), Some('c'))?;

    Ok(())
}

#[gtest]
fn char_after_slashes_slash() -> Result<()> {
    verify_eq!(char_after_slashes("/// doc"), Some('/'))?;

    Ok(())
}

#[gtest]
fn char_after_slashes_bare() -> Result<()> {
    verify_eq!(char_after_slashes("//"), None)?;

    Ok(())
}

#[gtest]
fn char_after_slashes_not_comment() -> Result<()> {
    verify_eq!(char_after_slashes("let x = 1;"), None)?;

    Ok(())
}

#[gtest]
fn prose_comment_content_normal() -> Result<()> {
    verify_eq!(prose_comment_content("// hello"), Some("hello"))?;

    Ok(())
}

#[gtest]
fn prose_comment_content_bare() -> Result<()> {
    verify_eq!(prose_comment_content("//"), Some(""))?;

    Ok(())
}

#[gtest]
fn prose_comment_content_not_comment() -> Result<()> {
    verify_eq!(prose_comment_content("let x = 1;"), None)?;

    Ok(())
}

#[gtest]
fn prose_comment_content_doc() -> Result<()> {
    verify_eq!(prose_comment_content("/// doc"), None)?;

    Ok(())
}

#[gtest]
fn redundant_field_name_parses() -> Result<()> {
    verify_eq!(
        redundant_field_name("redundant field initializer `x: x` — use shorthand `x`"),
        Some("x")
    )?;

    Ok(())
}

#[gtest]
fn redundant_field_name_none() -> Result<()> {
    verify_eq!(redundant_field_name("some other message"), None)?;

    Ok(())
}
