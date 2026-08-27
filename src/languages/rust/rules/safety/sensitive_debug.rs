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
        label: "Debug on struct with password field",
        code: "#[derive(Debug)]\nstruct Creds { password: String }",
        pass: false,
    },
    Example {
        label: "Debug on struct without sensitive fields",
        code: "#[derive(Debug)]\nstruct User { name: String }",
        pass: true,
    },
    Example {
        label: "no Debug with password field",
        code: "struct Creds { password: String }",
        pass: true,
    },
    Example {
        label: "Debug on struct with token field",
        code: "#[derive(Debug)]\nstruct Auth { token: String }",
        pass: false,
    },
    Example {
        label: "Debug on struct with api_key field",
        code: "#[derive(Debug)]\nstruct Config { api_key: String }",
        pass: false,
    },
    Example {
        label: "Debug on struct with bearer field",
        code: "#[derive(Debug)]\nstruct Auth { bearer: String }",
        pass: false,
    },
    Example {
        label: "marker at a field-name boundary",
        code: "#[derive(Debug)]\nstruct Auth { oauth_bearer: String }",
        pass: false,
    },
    Example {
        label: "marker text inside an unrelated word",
        code: "#[derive(Debug)]\nstruct Parser { tokenizer: String }",
        pass: true,
    },
    Example {
        label: "sensitive struct in test module",
        code: "#[cfg(test)]\nmod tests {\n    #[derive(Debug)]\n    struct Creds { password: String }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    sensitive_debug,
    "Flag `#[derive(Debug)]` on structs with sensitive fields like `password`.",
    "Deriving Debug on types with passwords or tokens risks leaking secrets in logs and error messages.",
    High,
    params {
        markers: [String] = [
            "api_key",
            "authorization",
            "bearer",
            "credential",
            "credentials",
            "password",
            "passwd",
            "private_key",
            "secret",
            "signing_key",
            "token",
        ]
    },
);

fn check_sensitive_debug(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let markers = sensitive_markers(ctx);

    ctx.nodes::<ast::Struct>()
        .filter(|item| !ctx.is_in_test(item) && has_debug_derive(item))
        .filter_map(|item| {
            let sensitive = find_sensitive_fields(&item, &markers);
            let name = item.name()?;

            (!sensitive.is_empty()).then(|| {
                ctx.violation(
                    &name,
                    format!(
                        "#[derive(Debug)] on struct with sensitive field(s): {} — implement Debug manually to redact",
                        sensitive.join(", ")
                    ),
                )
            })
        })
        .collect()
}

fn sensitive_markers(ctx: &AstCtx<'_>) -> Vec<String> {
    ctx.file
        .config
        .get_str_array("rust_sensitive_debug", &PARAMS[0])
}

fn is_sensitive_field(name: &str, markers: &[String]) -> bool {
    markers.iter().any(|marker| {
        name == marker
            || name
                .strip_prefix(marker)
                .is_some_and(|suffix| suffix.starts_with('_'))
            || name
                .strip_suffix(marker)
                .is_some_and(|prefix| prefix.ends_with('_'))
    })
}

fn has_debug_derive(item: &ast::Struct) -> bool {
    item.attrs().any(|attr| {
        attr.as_simple_call().is_some_and(|(name, tokens)| {
            name == "derive"
                && tokens
                    .syntax()
                    .text()
                    .to_string()
                    .strip_prefix('(')
                    .and_then(|text| text.strip_suffix(')'))
                    .is_some_and(|entries| entries.split(',').any(|entry| entry.trim() == "Debug"))
        })
    })
}

pub(crate) fn has_sensitive_fields(ctx: &AstCtx<'_>, item: &ast::Struct) -> bool {
    !find_sensitive_fields(item, &sensitive_markers(ctx)).is_empty()
}

fn find_sensitive_fields(item: &ast::Struct, markers: &[String]) -> Vec<String> {
    let Some(ast::FieldList::RecordFieldList(fields)) = item.field_list() else {
        return Vec::new();
    };

    let names = fields
        .fields()
        .filter_map(|field| field.name())
        .map(|name| name.text().to_string());

    names
        .filter(|name| is_sensitive_field(name, markers))
        .collect()
}

crate::rulewright_ast_test!(check_sensitive_debug, {
    crate::example_tests!(EXAMPLES, check_sensitive_debug);

    #[gtest]
    fn markers_are_configurable() -> Result<()> {
        let violations = crate::test_support::check_source_ast_params(
            "#[derive(Debug)]\nstruct Session { session_cookie: String }",
            "rust_sensitive_debug",
            &[("markers", &["session_cookie"])],
            check_sensitive_debug,
        );

        verify_eq!(violations.len(), 1)
    }
});
