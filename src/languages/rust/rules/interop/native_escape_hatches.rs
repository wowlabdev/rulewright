#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::ast::{self, HasName};
use std::collections::HashMap;

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "wrapper with all escape hatches",
        code: "pub struct Handle(*const u8);\nimpl Handle {\n    pub unsafe fn from_native(raw: *const u8) -> Self { Self(raw) }\n    pub fn into_native(self) -> *const u8 { self.0 }\n    pub fn to_native(&self) -> *const u8 { self.0 }\n}",
        pass: true,
    },
    Example {
        label: "wrapper without any escape hatch",
        code: "pub struct Handle(*mut u8);",
        pass: false,
    },
    Example {
        label: "from_native not unsafe",
        code: "pub struct Handle(*const u8);\nimpl Handle {\n    pub fn from_native(raw: *const u8) -> Self { Self(raw) }\n    pub fn into_native(self) -> *const u8 { self.0 }\n    pub fn to_native(&self) -> *const u8 { self.0 }\n}",
        pass: false,
    },
    Example {
        label: "missing to_native",
        code: "pub struct Handle(*const u8);\nimpl Handle {\n    pub unsafe fn from_native(raw: *const u8) -> Self { Self(raw) }\n    pub fn into_native(self) -> *const u8 { self.0 }\n}",
        pass: false,
    },
    Example {
        label: "private wrapper",
        code: "struct Handle(*const u8);",
        pass: true,
    },
    Example {
        label: "pointer plus length is not a plain wrapper",
        code: "pub struct Slice {\n    data: *const u8,\n    len: usize,\n}",
        pass: true,
    },
    Example {
        label: "non-pointer field",
        code: "pub struct Id(u64);",
        pass: true,
    },
    Example {
        label: "wrapper in test module",
        code: "#[cfg(test)]\nmod tests {\n    pub struct Handle(*const u8);\n}",
        pass: true,
    },
];

crate::ast_rule!(
    native_escape_hatches,
    "Require `unsafe fn from_native`, `into_native`, and `to_native` on public raw-pointer wrapper structs.",
    "Interop users need unsafe escape hatches to construct wrappers from native handles obtained elsewhere and to pass wrapped handles back over FFI.",
    Low,
);

fn check_native_escape_hatches(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let methods = collect_methods(ctx);

    let wrappers = ctx
        .nodes::<ast::Struct>()
        .filter(|item| !ctx.is_in_test(item))
        .filter(|item| super::support::is_fully_public(item) && is_raw_pointer_wrapper(item));

    wrappers
        .flat_map(|item| check_struct(ctx, &item, &methods))
        .collect()
}

// #rw(fn: rust_alloc_in_loop) method names are retained as owned cross-impl lookup keys
fn collect_methods(ctx: &AstCtx<'_>) -> HashMap<String, Vec<(String, bool)>> {
    let mut methods: HashMap<String, Vec<(String, bool)>> = HashMap::default();

    for item_impl in ctx
        .nodes::<ast::Impl>()
        .filter(|item| item.trait_().is_none())
    {
        let Some(name) = item_impl.self_ty().and_then(super::support::type_name) else {
            continue;
        };
        let Some(items) = item_impl.assoc_item_list() else {
            continue;
        };

        methods
            .entry(name)
            .or_default()
            .extend(items.assoc_items().filter_map(|item| match item {
                ast::AssocItem::Fn(function) => Some((
                    function.name()?.text().to_string(),
                    function.unsafe_token().is_some(),
                )),
                _ => None,
            }));
    }

    methods
}

fn is_raw_pointer_wrapper(item: &ast::Struct) -> bool {
    item.field_list().is_some_and(|fields| {
        let types = super::support::field_types(fields);

        types.len() == 1 && matches!(types.first(), Some(ast::Type::PtrType(_)))
    })
}

fn check_struct(
    ctx: &AstCtx<'_>,
    item: &ast::Struct,
    methods: &HashMap<String, Vec<(String, bool)>>,
) -> Vec<Violation> {
    let Some(name) = super::support::name_text(item) else {
        return Vec::new();
    };
    let item_methods = methods.get(&name);
    let has = |target: &str| item_methods.is_some_and(|m| m.iter().any(|(n, _)| n == target));
    let from_unsafe = item_methods.is_some_and(|m| {
        m.iter()
            .any(|(n, is_unsafe)| n == "from_native" && *is_unsafe)
    });
    let mut violations = Vec::new();

    if !has("from_native") {
        violations.push(ctx.violation(
            item,
            format!(
                "raw-pointer wrapper `{name}` is missing an `unsafe fn from_native` escape hatch"
            ),
        ));
    } else if !from_unsafe {
        violations.push(ctx.violation(
            item,
            format!("`{name}::from_native` must be an `unsafe fn` — constructing from a foreign native handle has safety requirements"),
        ));
    }

    if !has("into_native") {
        violations.push(ctx.violation(
            item,
            format!("raw-pointer wrapper `{name}` is missing an `into_native` escape hatch"),
        ));
    }

    if !has("to_native") {
        violations.push(ctx.violation(
            item,
            format!("raw-pointer wrapper `{name}` is missing a `to_native` escape hatch"),
        ));
    }

    violations
}

crate::rulewright_ast_test!(check_native_escape_hatches, {
    crate::example_tests!(EXAMPLES, check_native_escape_hatches);

    #[gtest]
    fn bare_wrapper_reports_all_three_methods() -> Result<()> {
        let v = run("pub struct Handle(*mut u8);");
        verify_eq!(v.len(), 3)?;

        Ok(())
    }
});
