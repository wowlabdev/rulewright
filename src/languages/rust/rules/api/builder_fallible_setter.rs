use ra_ap_syntax::{ast, ast::HasName};

use super::support::type_name;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "fallible setter",
        code: "struct HostBuilder;\nimpl HostBuilder {\n    fn port(self, port: u16) -> Result<Self, String> {\n        Ok(self)\n    }\n    fn build(self) -> u32 {\n        0\n    }\n}",
        pass: false,
    },
    Example {
        label: "fallible build",
        code: "struct HostBuilder;\nimpl HostBuilder {\n    fn port(self, port: u16) -> Self {\n        self\n    }\n    fn build(self) -> Result<u32, String> {\n        Ok(0)\n    }\n}",
        pass: true,
    },
    Example {
        label: "fallible try_build and finish",
        code: "struct HostBuilder;\nimpl HostBuilder {\n    fn try_build(self) -> Result<u32, String> {\n        Ok(0)\n    }\n    fn finish(self) -> Result<u32, String> {\n        Ok(0)\n    }\n}",
        pass: true,
    },
    Example {
        label: "fallible consuming terminal conversion",
        code: "struct HostBuilder; struct Draft; impl HostBuilder { fn into_draft(self) -> Result<Draft, String> { Ok(Draft) } }",
        pass: true,
    },
    Example {
        label: "fallible associated fn",
        code: "struct HostBuilder;\nimpl HostBuilder {\n    fn parse(input: &str) -> Result<Self, String> {\n        Ok(HostBuilder)\n    }\n}",
        pass: true,
    },
    Example {
        label: "fallible method on non-builder",
        code: "struct Config;\nimpl Config {\n    fn set(self, key: u32) -> Result<Self, String> {\n        Ok(self)\n    }\n}",
        pass: true,
    },
    Example {
        label: "fallible setter in test module",
        code: "#[cfg(test)]\nmod tests {\n    struct HostBuilder;\n    impl HostBuilder {\n        fn port(self, port: u16) -> Result<Self, String> {\n            Ok(self)\n        }\n    }\n}",
        pass: true,
    },
];

const ALLOWED_FALLIBLE: &[&str] = &["build", "try_build", "finish"];

crate::ast_rule!(
    builder_fallible_setter,
    "Flag builder setters returning `Result` — setters accept infallibly, validation belongs in `build()`.",
    "Fallible setters force repeated error checks that add noise and still cannot guard interdependent conditions; a Result-carrying build() consolidates validation.",
    Medium,
);

fn check_builder_fallible_setter(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let builders = ctx
        .nodes::<ast::Impl>()
        .filter(|item| !ctx.is_in_test(item) && item.trait_().is_none())
        .filter(|item| {
            item.self_ty()
                .is_some_and(|ty| type_name(&ty).is_some_and(|name| name.ends_with("Builder")))
        });

    builders
        .flat_map(|item| {
            let associated_items = item
                .assoc_item_list()
                .into_iter()
                .flat_map(|list| list.assoc_items());
            let setters = associated_items
                .filter_map(|assoc| match assoc {
                    ast::AssocItem::Fn(function) => Some(function),
                    _ => None,
                })
                .filter(|function| function.param_list().is_some_and(|params| params.self_param().is_some()));

            setters
                .filter(|function| {
                    let return_type = function
                        .ret_type()
                        .and_then(|ret| ret.ty())
                        .and_then(|ty| type_name(&ty));

                    return_type
                        .is_some_and(|name| name == "Result")
                })
                .filter_map(|function| {
                    let name = function.name()?;
                    let text = name.text().to_string();

                    (!ALLOWED_FALLIBLE.contains(&text.as_str()) && !text.starts_with("into_")).then(|| {
                        ctx.violation(
                            &name,
                            format!(
                                "fallible builder setter `{text}` — accept infallibly and validate in `build()`"
                            ),
                        )
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

crate::rulewright_ast_test!(check_builder_fallible_setter, {
    crate::example_tests!(EXAMPLES, check_builder_fallible_setter);
});
