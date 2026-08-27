use crate::{Example, TomlCtx, Violation, languages::unicode::ambiguous_unicode_violations};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example { label: "ambiguous punctuation in comment", code: "# range 1\u{2013}5\nvalue = 1\n", pass: false },
    Example { label: "homoglyph in string", code: "name = \"p\u{0430}ssword\"\n", pass: false },
    Example { label: "normal ASCII", code: "# range 1-5\nvalue = 1\n", pass: true },
    Example { label: "unambiguous non-ASCII", code: "# r\u{00E9}sum\u{00E9}\nvalue = 1\n", pass: true },
];

crate::toml_rule!(
    toml_ambiguous_unicode,
    "Ban Unicode characters in TOML that are visually confusable with ASCII.",
    "Confusable punctuation and homoglyphs make manifest comments and values easy to misread.",
    High,
);

fn check_toml_ambiguous_unicode(ctx: &TomlCtx<'_>) -> Vec<Violation> {
    ambiguous_unicode_violations(ctx.file)
}

crate::rulewright_toml_test!(check_toml_ambiguous_unicode, {
    crate::example_tests!(EXAMPLES, check_toml_ambiguous_unicode);
});
