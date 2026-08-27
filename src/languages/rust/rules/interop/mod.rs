//! Rust interoperability rules.

mod asref_bound_on_type;
mod concrete_io_param;
mod dll_boundary_types;
mod ffi_crate_naming;
mod ffi_in_core;
mod ffi_thin_glue;
mod foreign_reexports;
mod future_send_assert;
mod manual_async_fn;
mod native_escape_hatches;
mod nonsend_across_await;
mod owned_ref_param;
mod pub_api_foreign_types;
mod range_over_rangebounds;
mod support;

fn is_ffi_crate(name: &str) -> bool {
    name.ends_with("-ffi") || name.ends_with("_ffi")
}

fn is_sys_crate(name: &str) -> bool {
    name.ends_with("-sys") || name.ends_with("_sys")
}

/// wasm-bindgen exports follow their own ABI conventions, not the C FFI rules.
fn wasm_exempt(contents: &str) -> bool {
    contents.contains("wasm_bindgen")
}

/// `extern` without an explicit ABI defaults to `"C"`.
fn is_extern_c(abi: Option<ra_ap_syntax::ast::Abi>) -> bool {
    abi.is_some_and(|abi| {
        abi.string_token()
            .is_none_or(|name| name.text().trim_matches('"') == "C")
    })
}
