#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasAttrs, HasName},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "no_mangle extern C fn in core crate",
        code: "#[no_mangle]\npub extern \"C\" fn service_tick() {}",
        pass: false,
    },
    Example {
        label: "unsafe no_mangle extern C fn in core crate",
        code: "#[unsafe(no_mangle)]\npub extern \"C\" fn service_tick() {}",
        pass: false,
    },
    Example {
        label: "repr(C) struct with raw-pointer field",
        code: "#[repr(C)]\npub struct Msg {\n    data: *mut u8,\n    len: usize,\n}",
        pass: false,
    },
    Example {
        label: "repr(C) struct without pointers",
        code: "#[repr(C)]\npub struct Point {\n    x: f32,\n    y: f32,\n}",
        pass: true,
    },
    Example {
        label: "raw pointer without repr(C)",
        code: "pub struct Msg {\n    data: *mut u8,\n}",
        pass: true,
    },
    Example {
        label: "extern C fn without no_mangle",
        code: "pub extern \"C\" fn callback() {}",
        pass: true,
    },
    Example {
        label: "no_mangle without extern C abi",
        code: "#[no_mangle]\npub fn plain() {}",
        pass: true,
    },
    Example {
        label: "wasm_bindgen files follow their own convention",
        code: "use wasm_bindgen::prelude::JsValue;\n#[no_mangle]\npub extern \"C\" fn service_tick() {}",
        pass: true,
    },
    Example {
        label: "export in test module",
        code: "#[cfg(test)]\nmod tests {\n    #[no_mangle]\n    pub extern \"C\" fn service_tick() {}\n}",
        pass: true,
    },
];

crate::ast_rule!(
    ffi_in_core,
    "Flag `#[no_mangle] extern \"C\"` exports and `#[repr(C)]` raw-pointer structs in non-FFI crates.",
    "Business logic belongs in core crates as idiomatic safe Rust; interop concerns leaking into core force FFI ownership and data models onto everyone.",
    Medium,
);

fn check_ffi_in_core(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let Some(name) = ctx.file.package_name else {
        return Vec::new();
    };

    if super::is_ffi_crate(name)
        || super::is_sys_crate(name)
        || super::wasm_exempt(ctx.file.contents)
    {
        return Vec::new();
    }

    let ffi_functions = ctx
        .nodes::<ast::Fn>()
        .filter(|function| {
            super::support::is_item_or_impl_fn(function) && !ctx.is_in_test(function)
        })
        .filter(|function| has_no_mangle(function) && super::is_extern_c(function.abi()));
    let mut violations: Vec<_> = ffi_functions
        .filter_map(|function| {
            let name = function.name()?;

            Some(ctx.violation(
                &name,
                format!(
                    "C export `{name}` in a core crate — move FFI glue into a dedicated `*-ffi` crate"
                ),
            ))
        })
        .collect();

    violations.extend(
        {
            let ffi_structs = ctx
                .nodes::<ast::Struct>()
            .filter(|item| !ctx.is_in_test(item))
            .filter(|item| has_repr_c(item) && has_raw_pointer_field(item));

            ffi_structs
            .filter_map(|item| {
                let name = item.name()?;

                Some(ctx.violation(
                    &name,
                    format!(
                        "`#[repr(C)]` struct `{name}` with raw-pointer fields in a core crate — FFI data models belong in a dedicated `*-ffi` crate"
                    ),
                ))
            })
        },
    );

    violations
}

fn has_no_mangle(node: &impl HasAttrs) -> bool {
    node.attrs().any(|attr| {
        attr.simple_name().is_some_and(|name| name == "no_mangle")
            || attr
                .syntax()
                .to_string()
                .replace(' ', "")
                .contains("unsafe(no_mangle)")
    })
}

fn has_repr_c(node: &impl HasAttrs) -> bool {
    node.attrs().any(|attr| {
        attr.simple_name().is_some_and(|name| name == "repr")
            && attr
                .syntax()
                .descendants_with_tokens()
                .filter_map(ra_ap_syntax::NodeOrToken::into_token)
                .any(|token| token.text() == "C")
    })
}

fn has_raw_pointer_field(item: &ast::Struct) -> bool {
    item.field_list().is_some_and(|fields| {
        super::support::field_types(fields)
            .into_iter()
            .any(|ty| matches!(ty, ast::Type::PtrType(_)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CORE_REL: &str = "packages/core/src/lib.rs";

    fn run_at(rel: &str, source: &str) -> Vec<Violation> {
        crate::test_support::check_source_ast_at(rel, source, check_ffi_in_core)
    }

    #[gtest]
    fn examples() -> Result<()> {
        for ex in EXAMPLES {
            let violations = run_at(CORE_REL, ex.code);

            verify_eq!(violations.is_empty(), ex.pass)?;
        }

        Ok(())
    }

    #[gtest]
    fn ffi_sys_and_wasm_crates_are_exempt() -> Result<()> {
        let export = "#[no_mangle]\npub extern \"C\" fn service_tick() {}";

        verify_true!(run_at("nested/core-ffi/src/lib.rs", export).is_empty())?;
        verify_true!(run_at("nested/core_ffi/src/lib.rs", export).is_empty())?;
        verify_true!(run_at("nested/native-sys/src/lib.rs", export).is_empty())?;

        Ok(())
    }

    #[gtest]
    // #rw(fn: rust_duplicate_strings) This linter scenario intentionally repeats source fixtures used to verify independent paths.
    fn files_without_an_owning_package_are_skipped() -> Result<()> {
        verify_true!(run_at("clean.rs", "#[no_mangle]\npub extern \"C\" fn f() {}").is_empty())?;

        Ok(())
    }
}
