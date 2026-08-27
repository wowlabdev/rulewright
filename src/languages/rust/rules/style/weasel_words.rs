use ra_ap_syntax::ast;

use super::naming;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "Manager type",
        code: "struct BookingManager;",
        pass: false,
    },
    Example {
        label: "Service trait",
        code: "trait BookingService {}",
        pass: false,
    },
    Example {
        label: "Factory enum",
        code: "enum WidgetFactory { A }",
        pass: false,
    },
    Example {
        label: "Service type alias",
        code: "type AccountService = ();",
        pass: false,
    },
    Example {
        label: "weasel word in the middle",
        code: "struct ManagerConfig;",
        pass: false,
    },
    Example {
        label: "word only as partial segment",
        code: "struct Managed;",
        pass: true,
    },
    Example {
        label: "descriptive name",
        code: "struct BookingDispatcher;",
        pass: true,
    },
    Example {
        label: "use of a weasel type is not a definition",
        code: "use remote::BookingManager;",
        pass: true,
    },
    Example {
        label: "weasel-named fn is not a type definition",
        code: "fn manager() {}",
        pass: true,
    },
    Example {
        label: "weasel type in test module",
        code: "#[cfg(test)]\nmod tests {\n    struct FakeManager;\n}",
        pass: true,
    },
];

crate::ast_rule!(
    weasel_words,
    "Flag type definitions whose name contains a weasel word like `Manager`, `Service`, or `Factory`.",
    "Weasel words add no information: `Bookings` beats `BookingManager`, and Rust's name for a factory is `Builder`.",
    Medium,
    params {
        words: [String] = ["Factory", "Manager", "Service"]
    },
);

fn check_weasel_words(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let words = ctx
        .file
        .config
        .get_str_array("rust_weasel_words", &PARAMS[0]);

    ctx.nodes::<ast::Item>()
        .filter(|item| !ctx.is_in_test(item))
        .filter_map(|item| {
            let name = naming::type_def_name(&item)?;
            let name_text = name.text();
            let segments = naming::segments(&name_text);
            let word = words
                .iter()
                .find(|word| segments.iter().any(|segment| segment == *word))?;

            Some(ctx.violation(
                &name,
                format!(
                    "type name `{name_text}` contains weasel word `{word}` — name the type after what it is or does"
                ),
            ))
        })
        .collect()
}

crate::rulewright_ast_test!(check_weasel_words, {
    crate::example_tests!(EXAMPLES, check_weasel_words);
});
