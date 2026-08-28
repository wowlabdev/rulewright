#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "thin translation glue",
        code: "pub extern \"C\" fn transmit(data: *const u8, len: usize) -> u8 {\n    match core_transmit(data, len) {\n        Ok(()) => 0,\n        Err(_) => 1,\n    }\n}",
        pass: true,
    },
    Example {
        label: "extern C fn at the line threshold",
        code: "pub extern \"C\" fn at_limit() {
    let a01 = ();
    let a02 = ();
    let a03 = ();
    let a04 = ();
    let a05 = ();
    let a06 = ();
    let a07 = ();
    let a08 = ();
    let a09 = ();
    let a10 = ();
    let a11 = ();
    let a12 = ();
    let a13 = ();
    let a14 = ();
    let a15 = ();
    let a16 = ();
    let a17 = ();
    let a18 = ();
    let a19 = ();
    let a20 = ();
    let a21 = ();
    let a22 = ();
    let a23 = ();
    let a24 = ();
}",
        pass: true,
    },
    Example {
        label: "extern C fn over the line threshold",
        code: "pub extern \"C\" fn business_logic() {
    let a01 = ();
    let a02 = ();
    let a03 = ();
    let a04 = ();
    let a05 = ();
    let a06 = ();
    let a07 = ();
    let a08 = ();
    let a09 = ();
    let a10 = ();
    let a11 = ();
    let a12 = ();
    let a13 = ();
    let a14 = ();
    let a15 = ();
    let a16 = ();
    let a17 = ();
    let a18 = ();
    let a19 = ();
    let a20 = ();
    let a21 = ();
    let a22 = ();
    let a23 = ();
    let a24 = ();
    let a25 = ();
    let a26 = ();
}",
        pass: false,
    },
    Example {
        label: "long plain fn is not glue",
        code: "fn core_logic() {
    let a01 = ();
    let a02 = ();
    let a03 = ();
    let a04 = ();
    let a05 = ();
    let a06 = ();
    let a07 = ();
    let a08 = ();
    let a09 = ();
    let a10 = ();
    let a11 = ();
    let a12 = ();
    let a13 = ();
    let a14 = ();
    let a15 = ();
    let a16 = ();
    let a17 = ();
    let a18 = ();
    let a19 = ();
    let a20 = ();
    let a21 = ();
    let a22 = ();
    let a23 = ();
    let a24 = ();
    let a25 = ();
    let a26 = ();
}",
        pass: true,
    },
];

crate::ast_rule!(
    ffi_thin_glue,
    "Flag `extern \"C\"` functions in `*-ffi` crates whose body exceeds the line threshold.",
    "FFI glue only translates between C and Rust constructs; operational logic belongs in the core crate as idiomatic, safe, testable Rust.",
    Low,
    params {
        threshold: i64 = 25
    },
);

fn check_ffi_thin_glue(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let Some(name) = ctx.file.package_name else {
        return Vec::new();
    };

    if !super::is_ffi_crate(name) {
        return Vec::new();
    }

    let threshold = ctx
        .file
        .config
        .get_usize("rust_ffi_thin_glue", &FFI_THIN_GLUE_PARAMS[0]);

    ctx.nodes::<ast::Fn>()
        .filter(|function| {
            super::support::is_item_or_impl_fn(function)
                && !ctx.is_in_test(function)
                && super::is_extern_c(function.abi())
        })
        .filter_map(|function| {
            let body = function.body()?;
            let start = ctx.line_of(&body);
            let end = ctx
                .line_index
                .line_col(body.syntax().text_range().end())
                .line as usize
                + 1;
            let lines = end.saturating_sub(start);

            (lines > threshold).then(|| {
                let name = function.name()?;

                Some(ctx.violation(
                    &name,
                    format!(
                        "C glue fn `{name}` spans {lines} lines (max {threshold}) — glue translates, business logic belongs in the core crate"
                    ),
                ))
            })?
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FFI_REL: &str = "packages/core-ffi/src/lib.rs";

    fn run_at(rel: &str, source: &str) -> Vec<Violation> {
        crate::test_support::check_source_ast_at(rel, source, check_ffi_thin_glue)
    }

    #[gtest]
    fn examples() -> Result<()> {
        for ex in EXAMPLES {
            let violations = run_at(FFI_REL, ex.code);

            verify_eq!(violations.is_empty(), ex.pass)?;
        }

        Ok(())
    }

    #[gtest]
    fn underscore_ffi_crates_are_gated_too() -> Result<()> {
        let fail = EXAMPLES.iter().find(|ex| !ex.pass).or_fail()?;

        verify_false!(run_at("nested/core_ffi/src/lib.rs", fail.code).is_empty())?;

        Ok(())
    }

    #[gtest]
    // #rw(fn: rust_duplicate_strings) This linter scenario intentionally repeats source fixtures used to verify independent paths.
    fn non_ffi_crates_are_skipped() -> Result<()> {
        let fail = EXAMPLES.iter().find(|ex| !ex.pass).or_fail()?;

        verify_true!(run_at("nested/core/src/lib.rs", fail.code).is_empty())?;
        verify_true!(run_at("clean.rs", fail.code).is_empty())?;

        Ok(())
    }
}
