#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasAttrs, HasGenericArgs, HasName},
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
        label: "redacting sensitive wrapper",
        code: "#[derive(Debug)]\nstruct Config { api_key: Sensitive<String>, token: Option<SecretString> }",
        pass: true,
    },
    Example {
        label: "one redacted tuple member does not protect its siblings",
        code: "#[derive(Debug)]\nstruct Config { token: (String, SecretString) }",
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
    "Deriving Debug on types that actually contain credentials can leak them through logs and errors. Redact or implement Debug manually for real secrets; tune the sensitive-name list or suppress domain terms such as topology tokens rather than hiding useful non-secret diagnostics.",
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
        ],
        allowed_fields: [String] = [],
        redacted_types: [String] = [
            "Secret",
            "SecretBox",
            "SecretString",
            "SecretVec",
            "Sensitive",
        ],
    },
);

fn check_sensitive_debug(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let markers = sensitive_markers(ctx);
    let allowed = allowed_fields(ctx);
    let redacted = redacted_types(ctx);

    ctx.nodes::<ast::Struct>()
        .filter(|item| !ctx.is_in_test(item) && has_debug_derive(item))
        .filter_map(|item| {
            let sensitive = find_sensitive_fields(&item, &markers, &allowed, &redacted);
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

fn redacted_types(ctx: &AstCtx<'_>) -> Vec<String> {
    ctx.file
        .config
        .get_str_array("rust_sensitive_debug", &SENSITIVE_DEBUG_PARAMS[2])
}

fn allowed_fields(ctx: &AstCtx<'_>) -> Vec<String> {
    ctx.file
        .config
        .get_str_array("rust_sensitive_debug", &SENSITIVE_DEBUG_PARAMS[1])
}

fn sensitive_markers(ctx: &AstCtx<'_>) -> Vec<String> {
    ctx.file
        .config
        .get_str_array("rust_sensitive_debug", &SENSITIVE_DEBUG_PARAMS[0])
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
    !find_sensitive_fields(
        item,
        &sensitive_markers(ctx),
        &allowed_fields(ctx),
        &redacted_types(ctx),
    )
    .is_empty()
}

fn find_sensitive_fields(
    item: &ast::Struct,
    markers: &[String],
    allowed: &[String],
    redacted: &[String],
) -> Vec<String> {
    let Some(ast::FieldList::RecordFieldList(fields)) = item.field_list() else {
        return Vec::new();
    };

    fields
        .fields()
        .filter_map(|field| {
            let name = field.name()?.text().to_string();
            let sensitive = !allowed.contains(&name) && is_sensitive_field(&name, markers);

            (sensitive && !is_redacted_type(&field.ty()?, redacted)).then_some(name)
        })
        .collect()
}

fn is_redacted_type(ty: &ast::Type, redacted: &[String]) -> bool {
    let mut pending = vec![ty.clone()];

    while let Some(ty) = pending.pop() {
        match ty {
            ast::Type::PathType(path_type) => {
                let Some(segment) = path_type.path().and_then(|path| path.segment()) else {
                    return false;
                };
                let Some(name) = segment.name_ref() else {
                    return false;
                };

                if redacted.iter().any(|safe| safe == name.text().as_str()) {
                    continue;
                }

                if !matches!(
                    name.text().as_str(),
                    "Arc" | "Box" | "Option" | "Rc" | "Vec"
                ) {
                    return false;
                }

                let Some(arguments) = segment.generic_arg_list() else {
                    return false;
                };
                let types: Vec<ast::Type> = arguments
                    .generic_args()
                    .filter_map(|argument| {
                        let ast::GenericArg::TypeArg(argument) = argument else {
                            return None;
                        };

                        argument.ty()
                    })
                    .collect();
                let [inner] = types.as_slice() else {
                    return false;
                };

                pending.push(inner.clone());
            }

            ast::Type::RefType(reference) => {
                let Some(inner) = reference.ty() else {
                    return false;
                };

                pending.push(inner);
            }

            ast::Type::ParenType(parenthesized) => {
                let Some(inner) = parenthesized.ty() else {
                    return false;
                };

                pending.push(inner);
            }

            ast::Type::TupleType(tuple) => {
                let fields: Vec<ast::Type> = tuple.fields().collect();

                if fields.is_empty() {
                    return false;
                }

                pending.extend(fields);
            }

            _ => return false,
        }
    }

    true
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

    #[gtest]
    fn exact_domain_fields_can_be_allowed_without_disabling_the_marker() -> Result<()> {
        let source =
            "#[derive(Debug)]\nstruct Topology { topology_token: String, access_token: String }";
        let violations = crate::test_support::check_source_ast_params(
            source,
            "rust_sensitive_debug",
            &[("allowed_fields", &["topology_token"])],
            check_sensitive_debug,
        );

        verify_eq!(violations.len(), 1)?;
        verify_that!(
            violations[0].message.as_str(),
            contains_substring("access_token")
        )?;
        verify_that!(
            violations[0].message.as_str(),
            not(contains_substring("topology_token"))
        )
    }

    #[gtest]
    fn domain_redaction_wrappers_are_configurable() -> Result<()> {
        let violations = crate::test_support::check_source_ast_params(
            "#[derive(Debug)]\nstruct Work { claim_token: ClaimToken }",
            "rust_sensitive_debug",
            &[("redacted_types", &["ClaimToken"])],
            check_sensitive_debug,
        );

        verify_true!(violations.is_empty())
    }
});
