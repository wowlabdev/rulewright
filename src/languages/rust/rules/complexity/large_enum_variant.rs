use ra_ap_syntax::ast::{self, HasName};

use super::support;
use crate::{AstCtx, Example, Violation};

const MIN_VARIANT_COUNT: usize = 2;

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "enum with large variant",
        code: "enum Msg { Small(u8), Large([u8; 1024]) }",
        pass: false,
    },
    Example {
        label: "enum with similar-sized variants",
        code: "enum Msg { A(u64), B(u64) }",
        pass: true,
    },
    Example {
        label: "large variant boxed is fine",
        code: "enum Msg { Small(u8), Large(Box<[u8; 1024]>) }",
        pass: true,
    },
    Example {
        label: "large enum in test",
        code: "#[cfg(test)]\nmod tests {\n    enum Msg { Small(u8), Large([u8; 1024]) }\n}",
        pass: true,
    },
    Example {
        label: "single variant enum",
        code: "enum Single { Only([u8; 1024]) }",
        pass: true,
    },
    Example {
        label: "named-field large variant",
        code: "enum E { A { x: u8 }, B { data: [u64; 64] } }",
        pass: false,
    },
    Example {
        label: "unit variant vs large variant",
        code: "enum E { Empty, Big([u8; 512]) }",
        pass: false,
    },
];

crate::ast_rule!(
    large_enum_variant,
    "Flag enum variants that are much larger than others (should Box the large variant).",
    "All enum variants share the size of the largest. One huge variant wastes memory for every instance of the enum.",
    Medium,
    params {
        threshold: i64 = 256
    },
);

fn check_large_enum_variant(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let min_size_diff = ctx
        .file
        .config
        .get_u64("rust_large_enum_variant", &PARAMS[0]);
    let mut violations = Vec::new();

    for item in ctx
        .nodes::<ast::Enum>()
        .filter(|item| !ctx.is_in_test(item))
    {
        let Some(variants) = item.variant_list() else {
            continue;
        };
        let variants: Vec<_> = variants.variants().collect();

        if variants.len() < MIN_VARIANT_COUNT {
            continue;
        }

        let sizes: Vec<_> = variants
            .into_iter()
            .filter_map(|variant| {
                support::estimate_fields_size(variant.field_list()).map(|size| (variant, size))
            })
            .collect();

        if sizes.len() < MIN_VARIANT_COUNT {
            continue;
        }

        let min_size = sizes.iter().map(|(_, size)| *size).min().unwrap_or(0);

        for (variant, size) in sizes {
            if size > min_size + min_size_diff {
                let Some(name) = variant.name() else {
                    continue;
                };

                violations.push(ctx.violation(
                    &name,
                    format!(
                        "enum variant `{name}` is {size} bytes vs smallest {min_size} bytes — consider Boxing the large fields"
                    ),
                ));
            }
        }
    }

    violations
}

crate::rulewright_ast_test!(check_large_enum_variant, {
    crate::example_tests!(EXAMPLES, check_large_enum_variant);
});
