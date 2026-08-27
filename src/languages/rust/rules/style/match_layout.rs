#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{AstNode, Edition, SourceFile, ast, ast::HasArgList};

use crate::{AstCtx, Example, Violation};

use super::super::support::binary_macro_argument_candidates;

const COMPLEX_MATCHES: &str = "complex-matches";
const MULTILINE_ARM_SPACING: &str = "multiline-arm-spacing";
const MIN_COMPLEX_ALTERNATIVES: usize = 3;

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "simple matches macro",
        code: "fn valid(value: Value) -> bool { matches!(value, Value::One | Value::Two) }",
        pass: true,
    },
    Example {
        label: "complex multiline matches macro",
        code: "fn valid(pair: (Check, Fix)) -> bool {\n    matches!(\n        pair,\n        (\n            Check::Line,\n            None | Some(Fix::Line),\n        )\n            | (Check::Ast, Some(Fix::Ast))\n            | (Check::Toml, Some(Fix::Toml))\n    )\n}",
        pass: false,
    },
    Example {
        label: "separated multiline match arms",
        code: "fn run(value: Value) {\n    match value {\n        Value::One => {\n            prepare_one();\n            one();\n            finish_one();\n        }\n\n        Value::Two => {\n            prepare_two();\n            two();\n            finish_two();\n        }\n    }\n}",
        pass: true,
    },
    Example {
        label: "adjacent multiline match arms",
        code: "fn run(value: Value) {\n    match value {\n        Value::One => {\n            prepare_one();\n            one();\n            finish_one();\n        }\n        Value::Two => {\n            prepare_two();\n            two();\n            finish_two();\n        }\n    }\n}",
        pass: false,
    },
    Example {
        label: "compact arms",
        code: "fn value(input: bool) -> usize { match input { true => 1, false => 0 } }",
        pass: true,
    },
];

crate::ast_rule!(
    match_layout,
    "Keep match arms and `matches!` patterns visually structured.",
    "Complex pattern alternatives and dense multiline arms hide control flow; explicit match arms and whitespace make cases scannable.",
    Low,
    params {
        checks: [String] = [
            "complex-matches",
            "multiline-arm-spacing",
        ] in [
            COMPLEX_MATCHES,
            MULTILINE_ARM_SPACING,
        ],
    },
);

fn check_match_layout(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let checks = ctx
        .file
        .config
        .get_str_array("rust_match_layout", &PARAMS[0]);
    let mut violations = Vec::new();

    if enabled(&checks, COMPLEX_MATCHES) {
        violations.extend(complex_matches_violations(ctx));
    }

    for expression in ctx.nodes::<ast::MatchExpr>() {
        let Some(arms) = expression.match_arm_list() else {
            continue;
        };
        let arms: Vec<ast::MatchArm> = arms.arms().collect();

        if enabled(&checks, MULTILINE_ARM_SPACING) {
            violations.extend(multiline_spacing_violations(ctx, &arms));
        }
    }

    violations
}

fn enabled(checks: &[String], check: &str) -> bool {
    checks.iter().any(|configured| configured == check)
}

fn complex_matches_violations(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::MacroCall>()
        .filter(is_matches_macro)
        .filter_map(|call| {
            let pattern = matches_pattern(&call)?;

            complex_pattern(&pattern).then(|| {
                ctx.violation(
                    &call,
                    "multiline `matches!` has complex alternatives — use a `match` expression with one readable case per arm",
                )
            })
        })
        .collect()
}

fn is_matches_macro(call: &ast::MacroCall) -> bool {
    call.path().is_some_and(|path| {
        matches!(
            path.syntax()
                .text()
                .to_string()
                .replace(char::is_whitespace, "")
                .as_str(),
            "matches" | "std::matches" | "core::matches"
        )
    })
}

fn matches_pattern(call: &ast::MacroCall) -> Option<String> {
    binary_macro_argument_candidates(call)?
        .into_iter()
        .find_map(|(expression, mut pattern)| {
            pattern.truncate(pattern.trim_end().len());

            if pattern.ends_with(',') {
                pattern.pop();
            }

            (parse_expression(&expression).is_some() && parse_pattern(&pattern).is_some())
                .then_some(pattern)
        })
}

fn parse_expression(source: &str) -> Option<ast::Expr> {
    let wrapper = format!("fn __rulewright_match_layout() {{ let _ = {source}; }}");
    let parse = SourceFile::parse(&wrapper, Edition::Edition2024);

    if !parse.errors().is_empty() {
        return None;
    }

    parse
        .tree()
        .syntax()
        .descendants()
        .find_map(ast::LetStmt::cast)?
        .initializer()
}

fn complex_pattern(source: &str) -> bool {
    if !source.contains('\n') {
        return false;
    }

    let Some(ast::Pat::OrPat(pattern)) = parse_pattern(source) else {
        return false;
    };

    let alternatives: Vec<ast::Pat> = pattern.pats().collect();

    alternatives.len() >= MIN_COMPLEX_ALTERNATIVES
        && alternatives.iter().any(|alternative| {
            matches!(alternative, ast::Pat::TuplePat(_))
                && alternative.syntax().text().contains_char('\n')
        })
}

fn parse_pattern(source: &str) -> Option<ast::Pat> {
    let wrapper =
        format!("fn __rulewright_match_layout(value: ()) {{ match value {{ {source} => () }} }}");
    let parse = SourceFile::parse(&wrapper, Edition::Edition2024);

    if !parse.errors().is_empty() {
        return None;
    }

    parse
        .tree()
        .syntax()
        .descendants()
        .find_map(ast::MatchArm::cast)?
        .pat()
}

fn multiline_spacing_violations(ctx: &AstCtx<'_>, arms: &[ast::MatchArm]) -> Vec<Violation> {
    arms.windows(2)
        .filter_map(|pair| {
            let [previous, current] = pair else {
                return None;
            };
            let multiline = is_substantial_arm(previous) && is_substantial_arm(current);

            (multiline && !has_blank_line_between(ctx, previous, current)).then(|| {
                ctx.violation(
                    current,
                    "blank line required between adjacent match arms with substantial multiline bodies",
                )
            })
        })
        .collect()
}

fn is_substantial_arm(arm: &ast::MatchArm) -> bool {
    let Some(expression) = arm.expr() else {
        return false;
    };

    if !expression.syntax().text().contains_char('\n') {
        return false;
    }

    match expression {
        ast::Expr::BlockExpr(block) => block
            .stmt_list()
            .is_some_and(|statements| statements.statements().count() >= MIN_COMPLEX_ALTERNATIVES),
        ast::Expr::MethodCallExpr(expression) => {
            expression
                .syntax()
                .descendants()
                .filter_map(ast::MethodCallExpr::cast)
                .count()
                >= MIN_COMPLEX_ALTERNATIVES
        }
        ast::Expr::CallExpr(expression) => expression
            .arg_list()
            .is_some_and(|arguments| arguments.args().count() >= MIN_COMPLEX_ALTERNATIVES),

        ast::Expr::RecordExpr(expression) => expression
            .record_expr_field_list()
            .is_some_and(|fields| fields.fields().count() >= MIN_COMPLEX_ALTERNATIVES),

        ast::Expr::ClosureExpr(expression) => expression
            .body()
            .is_some_and(|body| matches!(body, ast::Expr::BlockExpr(_))),
        _ => false,
    }
}

fn has_blank_line_between(
    ctx: &AstCtx<'_>,
    previous: &ast::MatchArm,
    current: &ast::MatchArm,
) -> bool {
    let start: usize = previous.syntax().text_range().end().into();
    let end: usize = current.syntax().text_range().start().into();
    let Some(gap) = ctx.file.contents.get(start..end) else {
        return false;
    };

    gap.contains("\n\n") || gap.contains("\r\n\r\n")
}

crate::rulewright_ast_test!(check_match_layout, {
    crate::example_tests!(EXAMPLES, check_match_layout);

    #[gtest]
    fn two_multiline_matches_alternatives_remain_readable() -> Result<()> {
        let source = "fn valid(value: Value) -> bool {\n    matches!(\n        value,\n        Value::One\n            | Value::Two\n    )\n}";

        verify_true!(run(source).is_empty())
    }

    #[gtest]
    fn nested_pipes_do_not_count_as_top_level_alternatives() -> Result<()> {
        let source = "fn valid(value: Option<Value>) -> bool {\n    matches!(\n        value,\n        Some(Value::One | Value::Two | Value::Three)\n    )\n}";

        verify_true!(run(source).is_empty())
    }

    #[gtest]
    fn tuple_alternatives_remain_readable_when_each_stays_on_one_line() -> Result<()> {
        let source = "fn valid(pair: (Check, Fix)) -> bool {\n    matches!(\n        pair,\n        (Check::Line, Fix::Line)\n            | (Check::Ast, Fix::Ast)\n            | (Check::Toml, Fix::Toml)\n    )\n}";

        verify_true!(run(source).is_empty())
    }

    #[gtest]
    fn standard_qualified_matches_macros_are_checked() -> Result<()> {
        for macro_path in ["matches", "std::matches", "core::matches"] {
            let source = format!(
                "fn valid(pair: (Check, Fix)) -> bool {{\n    {macro_path}!(\n        pair,\n        (\n            Check::Line,\n            Fix::Line,\n        )\n            | (Check::Ast, Fix::Ast)\n            | (Check::Toml, Fix::Toml)\n    )\n}}"
            );

            verify_eq!(run(&source).len(), 1)?;
        }

        Ok(())
    }

    #[gtest]
    fn trailing_pattern_commas_are_checked_for_every_standard_macro_path() -> Result<()> {
        for macro_path in ["matches", "std::matches", "core::matches"] {
            let source = format!(
                "fn valid(pair: (Check, Fix)) -> bool {{\n    {macro_path}!(\n        pair,\n        (\n            Check::Line,\n            Fix::Line,\n        )\n            | (Check::Ast, Fix::Ast)\n            | (Check::Toml, Fix::Toml),\n    )\n}}"
            );

            verify_eq!(run(&source).len(), 1)?;
        }

        Ok(())
    }

    #[gtest]
    fn generic_commas_do_not_hide_the_pattern_argument() -> Result<()> {
        let source = "fn valid() -> bool {\n    matches!(\n        build::<Alpha, Beta>(),\n        (\n            Value::One,\n            Value::Two,\n        )\n            | (Value::Three, Value::Four)\n            | (Value::Five, Value::Six)\n    )\n}";

        verify_eq!(run(source).len(), 1)
    }

    #[gtest]
    fn const_generics_and_pattern_guards_keep_the_correct_split() -> Result<()> {
        let source = "fn valid() -> bool {\n    matches!(\n        build::<{ 1 }, { 2 }>(),\n        (\n            Value::One,\n            Value::Two,\n        )\n            | (Value::Three, Value::Four)\n            | (Value::Five, Value::Six) if enabled()\n    )\n}";

        verify_eq!(run(source).len(), 1)
    }

    #[gtest]
    fn adjacent_substantial_expression_arms_require_spacing() -> Result<()> {
        let source = "fn value(input: Input) -> usize {\n    match input {\n        Input::One(value) => value\n            .normalize()\n            .as_slice()\n            .len(),\n        Input::Two(value) => value\n            .normalize()\n            .as_slice()\n            .len(),\n    }\n}";

        verify_eq!(run(source).len(), 1)
    }

    #[gtest]
    fn checks_can_be_enabled_independently() -> Result<()> {
        let source = "fn valid(pair: (Check, Fix), value: Value) -> bool {\n    let compatible = matches!(\n        pair,\n        (\n            Check::Line,\n            Fix::Line,\n        )\n            | (Check::Ast, Fix::Ast)\n            | (Check::Toml, Fix::Toml)\n    );\n    match value {\n        Value::One => {\n            prepare_one();\n            run_one();\n            finish_one();\n        }\n        Value::Two => {\n            prepare_two();\n            run_two();\n            finish_two();\n        }\n    }\n    compatible\n}";
        let complex = crate::test_support::check_source_ast_params(
            source,
            "rust_match_layout",
            &[("checks", &[COMPLEX_MATCHES])],
            check_match_layout,
        );
        let spacing = crate::test_support::check_source_ast_params(
            source,
            "rust_match_layout",
            &[("checks", &[MULTILINE_ARM_SPACING])],
            check_match_layout,
        );
        let disabled = crate::test_support::check_source_ast_params(
            source,
            "rust_match_layout",
            &[("checks", &[])],
            check_match_layout,
        );

        verify_eq!(complex.len(), 1)?;
        verify_eq!(spacing.len(), 1)?;
        verify_true!(disabled.is_empty())
    }
});
