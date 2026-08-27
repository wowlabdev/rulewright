use crate::{Example, FileCtx, Violation, languages::unicode::ambiguous_unicode_violations};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example { label: "cyrillic a homoglyph", code: "let x = \"p\u{0430}ssword\";", pass: false },
    Example { label: "en dash instead of minus", code: "// range 1\u{2013}5", pass: false },
    Example { label: "curly apostrophe", code: "// don\u{2019}t", pass: false },
    Example { label: "multiplication sign", code: "// 4\u{00D7}4 matrix", pass: false },
    Example { label: "normal ASCII", code: "let x = \"hello world\";", pass: true },
    Example { label: "unambiguous non-ASCII", code: "// r\u{00E9}sum\u{00E9}", pass: true },
];

crate::line_rule!(
    ambiguous_unicode,
    "Ban Unicode characters visually confusable with ASCII (homoglyphs).",
    "Homoglyphs like Cyrillic U+0430 vs Latin 'a' make code read one way but compile another (trojan-source risk).",
    High,
);

fn check_ambiguous_unicode(ctx: &FileCtx<'_>) -> Vec<Violation> {
    ambiguous_unicode_violations(ctx)
}

crate::rulewright_test!(check_ambiguous_unicode, {
    crate::example_tests!(EXAMPLES, check_ambiguous_unicode);
});
