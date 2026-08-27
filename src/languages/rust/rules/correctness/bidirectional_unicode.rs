use crate::{Example, FileCtx, Violation, languages::unicode::bidirectional_unicode_violations};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "bidi LRE character",
        code: "let x = \"\u{202A}test\";",
        pass: false,
    },
    Example {
        label: "bidi RLO character",
        code: "let x = \"\u{202E}test\";",
        pass: false,
    },
    Example {
        label: "normal ASCII",
        code: "let x = \"hello world\";",
        pass: true,
    },
    Example {
        label: "bidi LRM character",
        code: "let x = \"\u{200E}test\";",
        pass: false,
    },
];

crate::line_rule!(
    bidirectional_unicode,
    "Ban Unicode bidi control characters that enable trojan-source attacks.",
    "Bidi control characters can reorder displayed code to hide malicious logic (CVE-2021-42574).",
    High,
);

fn check_bidirectional_unicode(ctx: &FileCtx<'_>) -> Vec<Violation> {
    bidirectional_unicode_violations(ctx)
}

crate::rulewright_test!(check_bidirectional_unicode, {
    crate::example_tests!(EXAMPLES, check_bidirectional_unicode);
});
