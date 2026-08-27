use ra_ap_syntax::ast;

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "cast to u8",
        code: "fn f() { let x = 42u64 as u8; }",
        pass: false,
    },
    Example {
        label: "cast to f32",
        code: "fn f() { let x = 1.0f64 as f32; }",
        pass: false,
    },
    Example {
        label: "cast to i16",
        code: "fn f() { let x = 1000i32 as i16; }",
        pass: false,
    },
    Example {
        label: "cast to u32",
        code: "fn f() { let x = 42u64 as u32; }",
        pass: true,
    },
    Example {
        label: "cast to u64",
        code: "fn f() { let x = 42u32 as u64; }",
        pass: true,
    },
    Example {
        label: "cast to usize",
        code: "fn f() { let x = 42u32 as usize; }",
        pass: true,
    },
    Example {
        label: "cast in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn f() { let x = 42u64 as u8; }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    lossy_cast,
    "Flag `as` casts to types that lose precision (`f32`, `u8`, `u16`, `i8`, `i16`).",
    "Casting to a smaller type (u64 as u8) silently truncates. Use try_into() to catch overflow at runtime.",
    Medium,
);

const LOSSY_TARGETS: &[&str] = &["f32", "u8", "u16", "i8", "i16"];

fn check_lossy_cast(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::CastExpr>()
        .filter(|cast| !ctx.is_in_test(cast))
        .filter_map(|cast| {
            let ast::Type::PathType(path) = cast.ty()? else {
                return None;
            };
            let ident = path.path()?.segment()?.name_ref()?;

            LOSSY_TARGETS
                .contains(&ident.text().as_str())
                .then(|| {
                    ctx.violation(
                        &cast,
                        format!(
                            "potentially lossy cast to `{ident}` — use explicit conversion (e.g. try_into(), try_from())"
                        ),
                    )
                })
        })
        .collect()
}

crate::rulewright_ast_test!(check_lossy_cast, {
    crate::example_tests!(EXAMPLES, check_lossy_cast);
});
