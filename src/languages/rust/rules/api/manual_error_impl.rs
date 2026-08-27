use ra_ap_syntax::{AstNode, ast};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example { label: "manual display", code: "struct ParseError; impl std::fmt::Display for ParseError { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { todo!() } }", pass: false },
    Example { label: "manual error", code: "struct ParseError; impl std::error::Error for ParseError {}", pass: false },
    Example { label: "thiserror", code: "#[derive(Debug, thiserror::Error)] #[error(\"bad\")] struct ParseError;", pass: true },
    Example { label: "non-error display", code: "struct Label; impl std::fmt::Display for Label { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { todo!() } }", pass: true },
    Example { label: "unrelated trait", code: "struct ParseError; impl Clone for ParseError { fn clone(&self) -> Self { Self } }", pass: true },
    Example { label: "test error", code: "#[cfg(test)] mod tests { struct StubError; impl std::error::Error for StubError {} }", pass: true },
];

crate::ast_rule!(
    manual_error_impl,
    "Reject hand-written `Display` and `Error` implementations for `*Error` types.",
    "Canonical errors derive `thiserror::Error` so formatting and source propagation remain declarative and consistent.",
    Low,
);

fn check_manual_error_impl(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::Impl>()
        .filter(|item_impl| !ctx.is_in_test(item_impl))
        .filter_map(|item_impl| {
            let self_path = item_impl
                .self_ty()
                .and_then(|ty| match ty {
                    ast::Type::PathType(path_type) => path_type.path(),
                    _ => None,
                })
                .and_then(|path| path.segment());
            let self_name = self_path
                .and_then(|segment| segment.name_ref())?;

            if !self_name.text().ends_with("Error") {
                return None;
            }

            let trait_type = item_impl.trait_()?;
            let normalized: String = trait_type
                .syntax()
                .text()
                .to_string()
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .collect();
            let trait_name = match normalized.as_str() {
                "Display" | "fmt::Display" | "std::fmt::Display" => "Display",
                "Error" | "error::Error" | "std::error::Error" => "std::error::Error",
                _ => return None,
            };

            Some(ctx.violation(
                &item_impl,
                format!(
                    "manual `{trait_name}` implementation for `{self_name}` — derive `thiserror::Error`"
                ),
            ))
        })
        .collect()
}

crate::rulewright_ast_test!(check_manual_error_impl, {
    crate::example_tests!(EXAMPLES, check_manual_error_impl);
});
