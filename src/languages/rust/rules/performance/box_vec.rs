#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::ast;

use crate::{AstCtx, Example, Violation};

use super::support::type_arg_last_ident;

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "Box<Vec<T>>",
        code: "fn f() { let x: Box<Vec<i32>> = Box::new(vec![]); }",
        pass: false,
    },
    Example {
        label: "Box<String>",
        code: "fn f() { let x: Box<String> = Box::new(String::new()); }",
        pass: false,
    },
    Example {
        label: "Box<Box<T>>",
        code: "fn f() { let x: Box<Box<i32>> = Box::new(Box::new(0)); }",
        pass: false,
    },
    Example {
        label: "Box<dyn Trait>",
        code: "fn f() { let x: Box<dyn Trait> = Box::new(foo); }",
        pass: true,
    },
    Example {
        label: "comment and literal with patterns",
        code: "// Box<Vec<i32>> is bad\nconst NOTE: &str = \"Box<String> and Box<Box<T>>\";",
        pass: true,
    },
];

crate::ast_rule!(
    box_vec,
    "Ban `Box<Vec<T>>`, `Box<String>`, `Box<Box<T>>` (unnecessary double indirection).",
    "Box<Vec<T>> adds a pointless heap indirection since Vec already heap-allocates. Use Vec<T> or Box<[T]>.",
    Medium,
);

fn check_box_vec(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::PathType>()
        .filter_map(|path_type| {
            let path = path_type.path()?;
            let outer = path.segment()?;

            (outer.name_ref()?.text() == "Box")
                .then(|| type_arg_last_ident(&outer))
                .flatten()
                .and_then(|inner| banned_message(&inner))
                .map(|message| ctx.violation(&path_type, message))
        })
        .collect()
}

fn banned_message(inner: &str) -> Option<&'static str> {
    match inner {
        "Vec" => Some("Box<Vec<T>> is double indirection (use Vec<T> directly)"),
        "String" => Some("Box<String> is double indirection (use String directly)"),
        "Box" => Some("Box<Box<T>> is double indirection (use Box<T> directly)"),
        _ => None,
    }
}

crate::rulewright_ast_test!(check_box_vec, {
    crate::example_tests!(EXAMPLES, check_box_vec);

    #[gtest]
    fn rule_is_deliberately_not_fixable() -> Result<()> {
        let metadata = crate::all_rules()
            .into_iter()
            .find(|rule| rule.name == "rust_box_vec")
            .or_fail()?;

        verify_false!(metadata.fixable)
    }

    #[gtest]
    fn comments_and_literals_do_not_look_like_types() -> Result<()> {
        let source = r##"
            // Box<Vec<i32>>
            const ONE: &str = "Box<String>";
            const TWO: &str = r#"Box<Box<u8>>"#;
        "##;

        verify_true!(run(source).is_empty())
    }
});
