use crate::{Example, TomlCtx, Violation, languages::unicode::bidirectional_unicode_violations};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example { label: "bidi control in string", code: "name = \"safe\u{202E}txt\"\n", pass: false },
    Example { label: "bidi control in comment", code: "# safe\u{2066}txt\nname = \"plain\"\n", pass: false },
    Example { label: "ordinary TOML", code: "name = \"plain\"\n", pass: true },
];

crate::toml_rule!(
    toml_bidirectional_unicode,
    "Ban Unicode bidi control characters in TOML.",
    "Bidi controls can reorder displayed manifests and comments to conceal dependency or configuration changes.",
    High,
);

fn check_toml_bidirectional_unicode(ctx: &TomlCtx<'_>) -> Vec<Violation> {
    bidirectional_unicode_violations(ctx.file)
}

crate::rulewright_toml_test!(check_toml_bidirectional_unicode, {
    crate::example_tests!(EXAMPLES, check_toml_bidirectional_unicode);
});
