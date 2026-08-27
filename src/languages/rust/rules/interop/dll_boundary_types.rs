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
        label: "String parameter in foreign block",
        code: "extern \"C\" {\n    fn take(s: String);\n}",
        pass: false,
    },
    Example {
        label: "Vec return from C export",
        code: "pub extern \"C\" fn give() -> Vec<u8> { Vec::new() }",
        pass: false,
    },
    Example {
        label: "boxed slice parameter",
        code: "pub extern \"C\" fn boxed(b: Box<[u8]>) {}",
        pass: false,
    },
    Example {
        label: "dyn trait reference parameter",
        code: "pub extern \"C\" fn cb(f: &dyn Fn()) {}",
        pass: false,
    },
    Example {
        label: "TypeId parameter in foreign block",
        code: "extern \"C\" {\n    fn id(t: TypeId);\n}",
        pass: false,
    },
    Example {
        label: "Instant parameter",
        code: "pub extern \"C\" fn when(t: std::time::Instant) {}",
        pass: false,
    },
    Example {
        label: "primitive and raw-pointer signature",
        code: "pub extern \"C\" fn ok(len: usize, data: *const u8) -> i32 { 0 }",
        pass: true,
    },
    Example {
        label: "portable foreign block",
        code: "extern \"C\" {\n    fn ok(x: u64) -> *mut u8;\n}",
        pass: true,
    },
    Example {
        label: "non-extern fn may use Rust types",
        code: "fn plain(s: String) {}",
        pass: true,
    },
    Example {
        label: "wasm_bindgen files follow their own convention",
        code: "use wasm_bindgen::JsValue;\npub extern \"C\" fn f(s: String) {}",
        pass: true,
    },
    Example {
        label: "C export in test module",
        code: "#[cfg(test)]\nmod tests {\n    pub extern \"C\" fn f(s: String) {}\n}",
        pass: true,
    },
];

crate::ast_rule!(
    dll_boundary_types,
    "Flag `String`, `Vec`, `Box`, `dyn` objects, `TypeId`, and `Instant` in `extern \"C\"` signatures.",
    "Each Rust DLL has its own statics, type layouts, and type ids, so only `#[repr(C)]`-style, primitive, or raw-pointer data is portable across the boundary.",
    High,
);

const BANNED: &[&str] = &["Box", "Instant", "String", "TypeId", "Vec"];

fn check_dll_boundary_types(ctx: &AstCtx<'_>) -> Vec<Violation> {
    if super::wasm_exempt(ctx.file.contents) {
        return Vec::new();
    }

    ctx.nodes::<ast::Fn>()
        .filter(|function| !ctx.is_in_test(function) && is_c_signature(function))
        .flat_map(|function| {
            let name = function.name().map(|name| name.text().to_string());
            let mut types: Vec<_> = function
                .param_list()
                .into_iter()
                .flat_map(|params| params.params())
                .filter_map(|param| param.ty())
                .collect();

            types.extend(function.ret_type().and_then(|output| output.ty()));

            types
                .into_iter()
                .filter_map(move |ty| {
                    nonportable(&ty).map(|bad| {
                        ctx.violation(
                            &ty,
                            format!(
                                "non-portable type `{bad}` in C signature of `{}` — only `#[repr(C)]`, primitive, or raw-pointer data may cross a DLL boundary",
                                name.as_deref().unwrap_or("<anonymous>")
                            ),
                        )
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn is_c_signature(function: &ast::Fn) -> bool {
    if function.abi().is_some() {
        return super::is_extern_c(function.abi());
    }

    function
        .syntax()
        .ancestors()
        .skip(1)
        .find_map(ast::ExternBlock::cast)
        .is_some_and(|block| super::is_extern_c(block.abi()))
}

fn nonportable(ty: &ast::Type) -> Option<&'static str> {
    if ty
        .syntax()
        .descendants()
        .any(|node| ast::DynTraitType::cast(node).is_some())
    {
        return Some("dyn Trait");
    }

    ty.syntax()
        .descendants()
        .filter_map(ast::PathType::cast)
        .filter_map(|path| path.path()?.segment()?.name_ref())
        .find_map(|name| BANNED.iter().copied().find(|banned| name.text() == *banned))
}

crate::rulewright_ast_test!(check_dll_boundary_types, {
    crate::example_tests!(EXAMPLES, check_dll_boundary_types);

    #[gtest]
    fn each_bad_parameter_is_flagged() -> Result<()> {
        let v = run("pub extern \"C\" fn f(a: String, b: Vec<u8>) -> Box<u8> { Box::new(0) }");
        verify_eq!(v.len(), 3)?;

        Ok(())
    }
});
