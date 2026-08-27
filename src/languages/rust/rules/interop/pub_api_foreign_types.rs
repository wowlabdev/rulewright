use ra_ap_syntax::{AstNode, ast};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "pub fn parameter of external type",
        code: "pub fn f(x: tokio::sync::Mutex<u32>) {}",
        pass: false,
    },
    Example {
        label: "pub fn returning external type",
        code: "pub fn f() -> serde_json::Value { make() }",
        pass: false,
    },
    Example {
        label: "external type nested in generic",
        code: "pub fn f(x: Vec<wasm_bindgen::JsValue>) {}",
        pass: false,
    },
    Example {
        label: "pub struct field of external type",
        code: "pub struct S { pub v: syn::Type }",
        pass: false,
    },
    Example {
        label: "pub enum variant of external type",
        code: "pub enum E { A(gix::ObjectId) }",
        pass: false,
    },
    Example {
        label: "pub type alias to external type",
        code: "pub type Alias = winnow::error::ErrMode<u32>;",
        pass: false,
    },
    Example {
        label: "pub method with external parameter",
        code: "pub struct S;\nimpl S {\n    pub fn f(&self, x: rayon::ThreadPool) {}\n}",
        pass: false,
    },
    Example {
        label: "private fn with external type",
        code: "fn f(x: tokio::sync::Mutex<u32>) {}",
        pass: true,
    },
    Example {
        label: "pub(crate) fn with external type",
        code: "pub(crate) fn f(x: syn::Type) {}",
        pass: true,
    },
    Example {
        label: "std type passes",
        code: "pub fn f(x: std::sync::Mutex<u32>) {}",
        pass: true,
    },
    Example {
        label: "workspace type passes",
        code: "pub fn f(x: vendor_types::RecordId) {}",
        pass: true,
    },
    Example {
        label: "bare imported name passes",
        code: "pub fn f(x: Value) {}",
        pass: true,
    },
    Example {
        label: "private field on pub struct passes",
        code: "pub struct S { v: syn::Type }",
        pass: true,
    },
    Example {
        label: "feature-gated fn passes",
        code: "#[cfg(feature = \"serde\")]\npub fn f(x: serde_json::Value) {}",
        pass: true,
    },
    Example {
        label: "feature-gated impl block passes",
        code: "pub struct S;\n#[cfg(feature = \"tokio\")]\nimpl S {\n    pub fn f(&self, x: tokio::net::TcpStream) {}\n}",
        pass: true,
    },
    Example {
        label: "feature-gated type alias passes",
        code: "#[cfg(any(feature = \"json\", test))]\npub type Alias = serde_json::Value;",
        pass: true,
    },
    Example {
        label: "pub fn in test module passes",
        code: "#[cfg(test)]\nmod tests {\n    pub fn f(x: syn::Type) {}\n}",
        pass: true,
    },
];

crate::ast_rule!(
    pub_api_foreign_types,
    "Flag foreign crate types leaked through `pub` fn signatures, fields, and type aliases.",
    "External types in public APIs tie the crate's stability to third-party crates; prefer std or workspace types. The external set is the owning crate's non-workspace [dependencies]; when no owning manifest exists (e.g. the test harness), `assume_external` supplies it.",
    Low,
    params {
        allowed: [String] = [],
        assume_external: [String] = [],
    },
);

// #rw(fn: rust_cyclomatic_complexity) each public API item shape has distinct typed-AST field access
fn check_pub_api_foreign_types(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let allowed = ctx
        .file
        .config
        .get_str_array("rust_pub_api_foreign_types", &PARAMS[0]);
    let mut external = match ctx
        .file
        .config
        .workspace()
        .external_dep_roots(ctx.file.path)
    {
        Some(roots) => roots.to_vec(),
        None => ctx
            .file
            .config
            .get_str_array("rust_pub_api_foreign_types", &PARAMS[1]),
    };

    external.retain(|root| !allowed.iter().any(|sanctioned| sanctioned == root));
    let mut violations = Vec::new();

    let public_functions = ctx
        .nodes::<ast::Fn>()
        .filter(|function| {
            super::support::is_item_or_impl_fn(function) && !ctx.is_in_test(function)
        })
        .filter(super::support::is_fully_public);

    for function in public_functions
        .filter(|function| !super::support::has_cfg_feature(function))
        .filter(|function| {
            super::support::enclosing_impl(function)
                .is_none_or(|item_impl| !super::support::has_cfg_feature(&item_impl))
        })
    {
        scan_signature(ctx, &function, &external, &mut violations);
    }

    for item in ctx
        .nodes::<ast::Struct>()
        .filter(|item| !ctx.is_in_test(item))
        .filter(|item| {
            super::support::is_fully_public(item) && !super::support::has_cfg_feature(item)
        })
    {
        if let Some(fields) = item.field_list() {
            match fields {
                ast::FieldList::RecordFieldList(fields) => {
                    for field in fields.fields().filter(super::support::is_fully_public) {
                        if let Some(ty) = field.ty() {
                            report(ctx, &ty, "struct field", &external, &mut violations);
                        }
                    }
                }
                ast::FieldList::TupleFieldList(fields) => {
                    for field in fields.fields().filter(super::support::is_fully_public) {
                        if let Some(ty) = field.ty() {
                            report(ctx, &ty, "struct field", &external, &mut violations);
                        }
                    }
                }
            }
        }
    }

    for item in ctx
        .nodes::<ast::Enum>()
        .filter(|item| !ctx.is_in_test(item))
        .filter(|item| {
            super::support::is_fully_public(item) && !super::support::has_cfg_feature(item)
        })
    {
        if let Some(variants) = item.variant_list() {
            for ty in variants
                .variants()
                .filter_map(|variant| variant.field_list())
                .flat_map(super::support::field_types)
            {
                report(ctx, &ty, "enum field", &external, &mut violations);
            }
        }
    }

    for item in ctx
        .nodes::<ast::TypeAlias>()
        .filter(|item| !ctx.is_in_test(item))
        .filter(|item| {
            super::support::is_fully_public(item) && !super::support::has_cfg_feature(item)
        })
    {
        if let Some(ty) = item.ty() {
            report(ctx, &ty, "type alias", &external, &mut violations);
        }
    }

    violations
}

fn foreign_roots(ty: &ast::Type, external: &[String]) -> Vec<(String, ast::NameRef)> {
    ty.syntax()
        .descendants()
        .filter_map(ast::Path::cast)
        .filter(|path| path.syntax().parent().and_then(ast::Path::cast).is_none())
        .filter_map(|path| {
            let names = super::support::path_names(path.clone());
            let root_name = names.first()?;

            external.iter().any(|name| name == root_name).then(|| {
                let root = path
                    .syntax()
                    .descendants()
                    .filter_map(ast::NameRef::cast)
                    .find(|name| name.text() == root_name.as_str())?;

                Some((root_name.clone(), root))
            })?
        })
        .collect()
}

fn report(
    ctx: &AstCtx<'_>,
    ty: &ast::Type,
    what: &str,
    external: &[String],
    violations: &mut Vec<Violation>,
) {
    for (name, root) in foreign_roots(ty, external) {
        violations.push(ctx.violation(
            &root,
            format!("public {what} leaks type from external crate `{name}`"),
        ));
    }
}

fn scan_signature(
    ctx: &AstCtx<'_>,
    function: &ast::Fn,
    external: &[String],
    violations: &mut Vec<Violation>,
) {
    if let Some(params) = function.param_list() {
        for ty in params.params().filter_map(|param| param.ty()) {
            report(ctx, &ty, "fn parameter", external, violations);
        }
    }

    if let Some(ty) = function.ret_type().and_then(|ret| ret.ty()) {
        report(ctx, &ty, "fn return type", external, violations);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str) -> Vec<Violation> {
        crate::test_support::check_source_ast_params(
            source,
            "rust_pub_api_foreign_types",
            &[(
                "assume_external",
                &[
                    "gix",
                    "js_sys",
                    "rayon",
                    "serde_json",
                    "syn",
                    "taplo",
                    "tokio",
                    "wasm_bindgen",
                    "web_sys",
                    "winnow",
                ],
            )],
            check_pub_api_foreign_types,
        )
    }

    crate::example_tests!(EXAMPLES, check_pub_api_foreign_types);
}
