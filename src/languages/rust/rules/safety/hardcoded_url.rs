use ra_ap_syntax::{
    AstNode, NodeOrToken, SyntaxKind,
    ast::{self, LiteralKind},
};

use super::super::support::is_in_const_or_static;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "hardcoded https URL",
        code: r#"fn f() { let url = "https://api.example.com/v1"; }"#,
        pass: false,
    },
    Example {
        label: "hardcoded http URL",
        code: r#"fn f() { let url = "http://localhost:3000"; }"#,
        pass: false,
    },
    Example {
        label: "no URL",
        code: "fn f() { let x = 42; }",
        pass: true,
    },
    Example {
        label: "URL in doc comment",
        code: "/// See https://docs.rs/foo for details.",
        pass: true,
    },
    Example {
        label: "URL in regular comment",
        code: "// Reference: https://example.com/spec",
        pass: true,
    },
    Example {
        label: "URL mentioned in a diagnostic",
        code: r#"fn f() { eprintln!("See https://example.com/help"); }"#,
        pass: true,
    },
    Example {
        label: "URL embedded in generated markup",
        code: r##"fn f() { let badge = r#"<a href="https://example.com">help</a>"#; }"##,
        pass: true,
    },
    Example {
        label: "scheme used as protocol syntax",
        code: r#"fn f(url: &str) { let _ = url.strip_prefix("https://"); }"#,
        pass: true,
    },
    Example {
        label: "URL prefix used as parser syntax",
        code: r#"fn f(url: &str) { let _ = url.strip_prefix("https://example.com/"); }"#,
        pass: true,
    },
    Example {
        label: "URL passed to a call before string matching",
        code: r#"fn f() { let _ = send("https://api.example.com").contains("ok"); }"#,
        pass: false,
    },
    Example {
        label: "URL-valued format string",
        code: r#"fn f() { let _ = format!("https://api.example.com/{path}"); }"#,
        pass: false,
    },
    Example {
        label: "named stable URL",
        code: r#"const PROJECT_SITE: &str = "https://example.com";"#,
        pass: true,
    },
    Example {
        label: "configuration fallback",
        code: r#"fn f(config: &Config) { let url = config.get_or("API_URL", "https://api.example.com"); }"#,
        pass: true,
    },
    Example {
        label: "URL in doc attribute",
        code: r#"#[doc = "See https://example.com/help"] fn f() {}"#,
        pass: true,
    },
];

crate::ast_rule!(
    hardcoded_url,
    "Flag URL-valued string literals compiled into source code (should use config/env).",
    "Environment-specific service URLs should not be compiled into behavior. Read hosts from configuration or inject them at the boundary; prose that merely mentions a URL and bare protocol syntax are not service endpoints. Stable URLs used as intentional identifiers may be scoped out with their purpose.",
    Medium,
);

fn check_hardcoded_url(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let literals = ctx
        .nodes::<ast::Literal>()
        .filter(|literal| !ctx.is_in_test(literal))
        .filter_map(|literal| {
            if literal
                .syntax()
                .ancestors()
                .any(|node| ast::Attr::can_cast(node.kind()))
            {
                return None;
            }

            let LiteralKind::String(text) = literal.kind() else {
                return None;
            };
            let value = text.value().ok()?;

            (starts_with_url_authority(value.trim_start())
                && !is_in_const_or_static(&literal)
                && !is_configuration_fallback(&literal)
                && !is_string_match_pattern(&literal))
                .then(|| {
                    ctx.violation(
                        &literal,
                        "hardcoded URL value in source — use configuration or environment variable instead",
                    )
                })
        });
    let macro_literals = ctx
        .nodes::<ast::MacroCall>()
        .filter(|call| !ctx.is_in_test(call))
        .filter(macro_has_url_value)
        .map(|call| {
            ctx.violation(
                &call,
                "hardcoded URL value in source — use configuration or environment variable instead",
            )
        });

    literals.chain(macro_literals).collect()
}

fn is_configuration_fallback(literal: &ast::Literal) -> bool {
    literal
        .syntax()
        .ancestors()
        .filter_map(ast::MethodCallExpr::cast)
        .filter_map(|call| call.name_ref())
        .any(|name| name.text() == "get_or")
}

fn starts_with_url_authority(value: &str) -> bool {
    ["https://", "http://"].iter().any(|scheme| {
        value
            .strip_prefix(scheme)
            .and_then(|rest| rest.chars().next())
            .is_some_and(|first| !first.is_whitespace())
    })
}

fn is_string_match_pattern(literal: &ast::Literal) -> bool {
    literal
        .syntax()
        .ancestors()
        .find_map(ast::ArgList::cast)
        .and_then(|arguments| arguments.syntax().parent())
        .and_then(ast::MethodCallExpr::cast)
        .and_then(|call| call.name_ref())
        .is_some_and(|name| {
            matches!(
                name.text().as_str(),
                "contains"
                    | "ends_with"
                    | "starts_with"
                    | "strip_prefix"
                    | "strip_suffix"
                    | "trim_end_matches"
                    | "trim_start_matches"
            )
        })
}

fn macro_has_url_value(call: &ast::MacroCall) -> bool {
    call.token_tree().is_some_and(|tokens| {
        tokens
            .syntax()
            .descendants_with_tokens()
            .filter_map(NodeOrToken::into_token)
            .filter(|token| token.kind() == SyntaxKind::STRING)
            .any(|token| {
                string_token_value(token.text())
                    .is_some_and(|value| starts_with_url_authority(value.trim_start()))
            })
    })
}

fn string_token_value(token: &str) -> Option<&str> {
    let first_quote = token.find('"')?;
    let last_quote = token.rfind('"')?;

    if first_quote >= last_quote {
        return None;
    }

    token.get(first_quote + 1..last_quote)
}

crate::rulewright_ast_test!(check_hardcoded_url, {
    crate::example_tests!(EXAMPLES, check_hardcoded_url);
});
