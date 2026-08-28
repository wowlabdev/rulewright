use ra_ap_syntax::ast;

use super::naming;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "five-word struct name",
        code: "struct GlobalApplicationConfigManagerImpl;",
        pass: false,
    },
    Example {
        label: "five-word enum name",
        code: "enum BookingRequestQueueItemState { A }",
        pass: false,
    },
    Example {
        label: "five-word trait name",
        code: "trait AsyncRemoteAccountDataSource {}",
        pass: false,
    },
    Example {
        label: "five-word type alias",
        code: "type SharedRemoteAccountDataCache = ();",
        pass: false,
    },
    Example {
        label: "two-word name",
        code: "struct AppConfig;",
        pass: true,
    },
    Example {
        label: "four words is at the threshold",
        code: "struct HtmlParserConfigBuilder;",
        pass: true,
    },
    Example {
        label: "acronym run counts as one word",
        code: "struct HTMLParserConfigBuilder;",
        pass: true,
    },
    Example {
        label: "long name in test module",
        code: "#[cfg(test)]\nmod tests {\n    struct GlobalApplicationConfigManagerFake;\n}",
        pass: true,
    },
];

crate::ast_rule!(
    long_compound_name,
    "Flag type definitions whose CamelCase name compounds more than threshold words.",
    "Long compound names can bury the item's essential role. Remove words already supplied by the module or type context, but keep enough domain meaning to avoid vague names and collisions; suppress a deliberately disambiguated public name rather than weakening the API.",
    Low,
    params { threshold: i64 = 4 },
);

fn check_long_compound_name(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let threshold = ctx
        .file
        .config
        .get_usize("rust_long_compound_name", &LONG_COMPOUND_NAME_PARAMS[0]);

    ctx.nodes::<ast::Item>()
        .filter(|item| !ctx.is_in_test(item))
        .filter_map(|item| {
            let name = naming::type_def_name(&item)?;
            let name_text = name.text();
            let words = naming::segments(&name_text).len();

            (words > threshold).then(|| {
                ctx.violation(
                    &name,
                    format!(
                        "type name `{name_text}` compounds {words} words (max {threshold}) — shorten it"
                    ),
                )
            })
        })
        .collect()
}

crate::rulewright_ast_test!(check_long_compound_name, {
    crate::example_tests!(EXAMPLES, check_long_compound_name);
});
