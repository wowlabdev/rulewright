use ra_ap_syntax::ast;

use super::support;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "large stack array",
        code: "fn f() { let _buf: [u8; 8192] = [0u8; 8192]; }",
        pass: false,
    },
    Example {
        label: "small stack array",
        code: "fn f() { let _buf = [0u8; 64]; }",
        pass: true,
    },
    Example {
        label: "large array in test",
        code: "#[cfg(test)]\nmod tests {\n    fn t() { let _buf = [0u8; 8192]; }\n}",
        pass: true,
    },
    Example {
        label: "u64 element array type",
        code: "struct S { buf: [u64; 1024] }",
        pass: false,
    },
    Example {
        label: "boxed element array type",
        code: "struct S { buf: [Box<u32>; 1024] }",
        pass: false,
    },
    Example {
        label: "tuple element array type",
        code: "struct S { buf: [(u64, u32); 1024] }",
        pass: false,
    },
    Example {
        label: "reference element array type",
        code: "struct S<'a> { buf: [&'a u8; 1024] }",
        pass: false,
    },
    Example {
        label: "f64 repeat literal array",
        code: "fn f() { let _b = [0.0f64; 1024]; }",
        pass: false,
    },
    Example {
        label: "u64 repeat literal array",
        code: "fn f() { let _b = [0u64; 1024]; }",
        pass: false,
    },
];

crate::ast_rule!(
    large_stack_array,
    "Flag large fixed-size arrays on the stack (>threshold bytes). WASM has limited stack.",
    "WASM has a fixed 1MB stack by default. Large stack arrays can silently overflow it and crash at runtime.",
    High,
    params {
        threshold: i64 = 4096
    },
);

fn check_large_stack_array(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let max_stack_bytes = ctx
        .file
        .config
        .get_u64("rust_large_stack_array", &LARGE_STACK_ARRAY_PARAMS[0]);
    let type_arrays = ctx
        .nodes::<ast::ArrayType>()
        .filter(|array| !ctx.is_in_test(array))
        .filter_map(|array| {
            let size = support::estimate_type_size(&ast::Type::ArrayType(array.clone()))?;

            (size > max_stack_bytes).then(|| {
                ctx.violation(
                    &array,
                    format!(
                        "large stack array ({size} bytes) — consider Box<[T; N]> or Vec<T> (WASM stack is limited)"
                    ),
                )
            })
        });
    let repeat_arrays = ctx
        .nodes::<ast::ArrayExpr>()
        .filter(|array| !ctx.is_in_test(array) && array.semicolon_token().is_some())
        .filter_map(|array| {
            let mut expressions = array.exprs();
            let element = expressions.next()?;
            let length = expressions.next()?;
            let total = support::literal_elem_size(&element)?
                .saturating_mul(support::parse_int_expr(&length)?);

            (total > max_stack_bytes).then(|| {
                ctx.violation(
                    &array,
                    format!(
                        "large stack array ({total} bytes) — consider Box<[T; N]> or Vec<T> (WASM stack is limited)"
                    ),
                )
            })
        });

    type_arrays.chain(repeat_arrays).collect()
}

crate::rulewright_ast_test!(check_large_stack_array, {
    crate::example_tests!(EXAMPLES, check_large_stack_array);
});
