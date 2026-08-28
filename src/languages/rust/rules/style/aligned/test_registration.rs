use super::*;

crate::rulewright_test!(check_aligned, {
    crate::example_tests!(EXAMPLES, check_aligned);
    crate::fix_tests!(EXAMPLES, line, check_aligned, fix_aligned);

    #[gtest]
    fn arrow_aligned_passes() -> Result<()> {
        let v = run("// #rw:aligned\n\
             Parser    => \"cleanup_parser.sql\";\n\
             Writer    => \"cleanup_traits.sql\";\n\
             Files     => \"cleanup_items.sql\";");
        verify_true!(v.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn arrow_misaligned_fails() -> Result<()> {
        let v = run("// #rw:aligned\n\
             Parser    => \"cleanup_parser.sql\";\n\
             Writer => \"cleanup_traits.sql\";\n\
             Files     => \"cleanup_items.sql\";");
        verify_eq!(v.len(), 1)?;
        verify_eq!(v[0].line, 3)?;
        verify_true!(v[0].message.contains("=>"))?;

        Ok(())
    }

    #[gtest]
    fn comma_aligned_passes() -> Result<()> {
        let v = run("// #rw:aligned\n\
             register!(A,  \"a\",  TypeA);\n\
             register!(B,  \"b\",  TypeB);\n\
             register!(C,  \"c\",  TypeC);");
        verify_true!(v.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn comma_misaligned_fails() -> Result<()> {
        let v = run("// #rw:aligned\n\
             register!(A,  \"a\",  TypeA);\n\
             register!(B, \"b\", TypeB);\n\
             register!(C,  \"c\",  TypeC);");
        verify_false!(v.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn trailing_commas_are_aligned() -> Result<()> {
        let source = "// #rw:aligned\n\
             \"id\": Int,\n\
             \"patch_version\": Text,\n\
             \"cloth_modifier\": Float,";
        let fixed = crate::apply_line_fixes(source, check_aligned, fix_aligned);

        verify_true!(run(&fixed).is_empty())?;

        Ok(())
    }

    #[gtest]
    fn skips_comments_in_block() -> Result<()> {
        let v = run("// #rw:aligned\n\
             Parser => \"a\";\n\
             // comment\n\
             Writer => \"b\";");
        verify_true!(v.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn block_ends_at_blank_line() -> Result<()> {
        let v = run("// #rw:aligned\n\
             Parser => \"a\";\n\
             Writer => \"b\";\n\
             \n\
             Misaligned => \"c\";");
        verify_true!(v.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn block_ends_at_closing_delim() -> Result<()> {
        let v = run("// #rw:aligned\n\
             Parser => \"a\";\n\
             Writer => \"b\";\n\
             );");
        verify_true!(v.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn block_ends_at_array_closing_delim() -> Result<()> {
        let v = run("// #rw:aligned\n\
             (1, 2),\n\
             (3, 4),\n\
             ];\n\
             call(a, b, c);");
        verify_true!(v.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn block_ends_at_macro_array_closing_delim() -> Result<()> {
        let v = run("// #rw:aligned\n\
             \"id\": Int,\n\
             \"x\": Text,\n\
             ]);");
        verify_true!(v.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn no_marker_no_violations() -> Result<()> {
        let v = run("Parser    => \"a\";\n\
             Writer => \"b\";");
        verify_true!(v.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn marker_inside_raw_string_is_ignored() -> Result<()> {
        let source = r##"const SOURCE: &str = r#"
// #rw:aligned
(SHORT, "first"),
(MUCH_LONGER_NAME, "second"),
"#;"##;

        verify_true!(run(source).is_empty())
    }

    #[gtest]
    fn comma_inside_string_ignored() -> Result<()> {
        let v = run("// #rw:aligned\n\
             call(a, \"hello, world\", b);\n\
             call(c, \"hello, world\", d);");
        verify_true!(v.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn mismatched_comma_count() -> Result<()> {
        let v = run("// #rw:aligned\n\
             call(a, b, c);\n\
             call(a, b, c);\n\
             call(a, b);");
        verify_eq!(v.len(), 1)?;
        verify_true!(v[0].message.contains("aligned fields"))?;

        Ok(())
    }

    #[gtest]
    fn wrapped_tuple_rows_fix_to_one_aligned_row_each() -> Result<()> {
        let source = "let cases = [\n    // #rw:aligned\n    (\n        SHORT,\n        \"first\",\n    ),\n    (LONG_NAME, \"second\"),\n];";
        let expected = "let cases = [\n    // #rw:aligned\n    (SHORT,     \"first\"),\n    (LONG_NAME, \"second\"),\n];";

        verify_eq!(
            crate::apply_line_fixes(source, check_aligned, fix_aligned),
            expected
        )
    }

    #[gtest]
    fn longest_tuple_field_sets_the_aligned_column() -> Result<()> {
        let source = "let cases = [\n    // #rw:aligned\n    (SHORT,            \"first\"),\n    (MUCH_LONGER_NAME, \"second\"),\n    (MEDIUM,           \"third\"),\n];";
        let misaligned = "let cases = [\n    // #rw:aligned\n    (SHORT, \"first\"),\n    (MUCH_LONGER_NAME, \"second\"),\n    (MEDIUM, \"third\"),\n];";

        verify_false!(run(misaligned).is_empty())?;
        verify_eq!(
            crate::apply_line_fixes(misaligned, check_aligned, fix_aligned),
            source
        )?;

        verify_true!(run(source).is_empty())
    }

    #[gtest]
    fn wrapped_tuple_with_nested_expression_has_no_fix() -> Result<()> {
        let source = "let cases = [\n    // #rw:aligned\n    (\n        SHORT,\n        build(value),\n    ),\n    (LONG_NAME, other),\n];";

        verify_false!(run(source).is_empty())?;
        verify_eq!(
            crate::apply_line_fixes(source, check_aligned, fix_aligned),
            source
        )
    }

    #[gtest]
    fn wrapped_tuple_with_block_expression_has_no_fix() -> Result<()> {
        let source = "let cases = [\n    // #rw:aligned\n    (\n        SHORT,\n        { compute() },\n    ),\n    (LONG_NAME, other),\n];";

        verify_false!(run(source).is_empty())?;
        verify_eq!(
            crate::apply_line_fixes(source, check_aligned, fix_aligned),
            source
        )
    }

    #[gtest]
    fn majority_ties_choose_the_smaller_column() -> Result<()> {
        verify_eq!(majority([12, 8].into_iter()), 8)?;

        let violations = run("// #rw:aligned\nShort => one,\nLongName => two,");

        verify_eq!(violations.len(), 1)?;
        verify_eq!(violations[0].line, 3)
    }
});
