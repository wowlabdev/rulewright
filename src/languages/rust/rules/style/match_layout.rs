#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{AstNode, ast};

use crate::{AstCtx, Example, Violation};

use super::support::{gap_before_attached_comment, has_blank_line};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "small compact match",
        code: "fn value(input: bool) -> usize { match input { true => 1, false => 0 } }",
        pass: true,
    },
    Example {
        label: "large match with compact one-line arms",
        code: "fn value(input: Input) -> usize {\n    match input {\n        Input::One => 1,\n        Input::Two => 2,\n        Input::Three => 3,\n        Input::Four => 4,\n        Input::Five => 5,\n        Input::Six => 6,\n        Input::Seven => 7,\n        Input::Eight => 8,\n        Input::Nine => 9,\n        Input::Ten => 10,\n    }\n}",
        pass: true,
    },
    Example {
        label: "one-line arms with unnecessary gaps",
        code: "fn value(input: Input) -> usize {\n    match input {\n        Input::One => 1,\n\n        Input::Two => 2,\n\n        Input::Three => 3,\n    }\n}",
        pass: false,
    },
    Example {
        label: "one multiline body makes the whole match spacious",
        code: "fn run(input: Input) {\n    match input {\n        Input::One => {\n            prepare_one();\n            finish_one();\n        }\n        Input::Two => finish(),\n        Input::Three => finish_three(),\n    }\n}",
        pass: false,
    },
    Example {
        label: "multiline match with inconsistent separation",
        code: "fn run(input: Input) {\n    match input {\n        Input::One => {\n            one();\n        }\n\n        Input::Two => two(),\n        Input::Three => three(),\n    }\n}",
        pass: false,
    },
];

crate::ast_tree_rule!(
    match_layout,
    "Keep one-line match arms compact, collapse empty arm blocks, and separate every arm when any body is multiline.",
    "Uniform one-line arms are easiest to scan as a compact list, empty bodies should be `{}`, and a multiline body needs one blank line between every arm so the cases remain visually distinct.",
    Low,
    fix_match_layout,
);

fn check_match_layout(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::MatchExpr>()
        .filter_map(|expression| {
            let arm_count = expression.match_arm_list()?.arms().count();
            let fixes = gap_fixes(&expression);
            let expanded_empty = expression
                .match_arm_list()
                .is_some_and(|list| list.arms().any(|arm| expanded_empty_block(&arm).is_some()));

            (expanded_empty || !fixes.is_empty()).then(|| {
                let spacious = is_spacious(&expression);
                let problem = if expanded_empty {
                    "contains an empty arm body that should be written as `{}`; shorten the pattern or import its type if rustfmt expands it"
                } else if spacious {
                    "needs a blank line between every arm"
                } else {
                    "contains only one-line arms and should not have blank lines between them"
                };

                ctx.violation(
                    &expression,
                    format!("match with {arm_count} arms {problem}"),
                )
            })
        })
        .collect()
}

#[derive(Clone, Copy)]
enum GapFix {
    Add,
    Remove,
}

fn gap_fixes(expression: &ast::MatchExpr) -> Vec<(ast::MatchArm, GapFix)> {
    let Some(list) = expression.match_arm_list() else {
        return Vec::new();
    };
    let arms: Vec<ast::MatchArm> = list.arms().collect();
    let spacious = arms.iter().any(is_multiline);

    arms.into_iter()
        .skip(1)
        .filter_map(|arm| {
            let has_gap =
                gap_before_attached_comment(arm.syntax()).is_some_and(|gap| has_blank_line(&gap));

            match (spacious, has_gap) {
                (true, false) => Some((arm, GapFix::Add)),
                (false, true) => Some((arm, GapFix::Remove)),
                _ => None,
            }
        })
        .collect()
}

fn is_multiline(arm: &ast::MatchArm) -> bool {
    if expanded_empty_block(arm).is_some() {
        return false;
    }

    arm.expr()
        .is_some_and(|expression| expression.syntax().text().to_string().contains('\n'))
}

fn expanded_empty_block(arm: &ast::MatchArm) -> Option<ast::BlockExpr> {
    let ast::Expr::BlockExpr(block) = arm.expr()? else {
        return None;
    };
    let text = block.syntax().text().to_string();
    let inner = text.strip_prefix('{')?.strip_suffix('}')?;

    (inner.trim().is_empty() && inner.contains('\n')).then_some(block)
}

fn is_spacious(expression: &ast::MatchExpr) -> bool {
    expression
        .match_arm_list()
        .is_some_and(|list| list.arms().any(|arm| is_multiline(&arm)))
}

fn fix_match_layout(ctx: &AstCtx<'_>, _violations: &[Violation]) -> Option<String> {
    let mut edits: Vec<(usize, usize, String)> = Vec::new();

    for expression in ctx
        .root
        .syntax()
        .descendants()
        .filter_map(ast::MatchExpr::cast)
    {
        if let Some(list) = expression.match_arm_list() {
            for block in list.arms().filter_map(|arm| expanded_empty_block(&arm)) {
                let range = block.syntax().text_range();

                edits.push((range.start().into(), range.end().into(), "{}".to_owned()));
            }
        }

        for (arm, fix) in gap_fixes(&expression) {
            let Some(gap) = gap_before_attached_comment(arm.syntax()) else {
                continue;
            };
            let replacement = match fix {
                GapFix::Add => format!("\n{}", gap.text()),

                GapFix::Remove => {
                    let indentation = gap.text().rsplit_once('\n').map_or("", |(_, tail)| tail);

                    format!("\n{indentation}")
                }
            };
            let range = gap.text_range();

            edits.push((range.start().into(), range.end().into(), replacement));
        }
    }

    if edits.is_empty() {
        return None;
    }

    edits.sort_unstable_by_key(|edit| std::cmp::Reverse(edit.0));

    let mut fixed = ctx.file.contents.to_owned();

    for (start, end, replacement) in edits {
        fixed.replace_range(start..end, &replacement);
    }

    Some(fixed)
}

crate::rulewright_ast_test!(check_match_layout, {
    crate::example_tests!(EXAMPLES, check_match_layout);
    crate::fix_tests!(EXAMPLES, ast_tree, check_match_layout, fix_match_layout);

    #[gtest]
    fn each_affected_match_reports_once() -> Result<()> {
        verify_eq!(run(EXAMPLES[2].code).len(), 1)?;

        verify_eq!(run(EXAMPLES[3].code).len(), 1)?;

        verify_eq!(run(EXAMPLES[4].code).len(), 1)
    }

    #[gtest]
    fn fix_keeps_comments_and_attributes_with_the_following_arm() -> Result<()> {
        let source = "fn run(input: Input) {\n    match input {\n        Input::One => {\n            prepare();\n            finish();\n        }\n        // The fallback must remain documented.\n        #[cfg(unix)]\n        Input::Two => {\n            prepare_fallback();\n            finish();\n        }\n    }\n}";
        let expected = "fn run(input: Input) {\n    match input {\n        Input::One => {\n            prepare();\n            finish();\n        }\n\n        // The fallback must remain documented.\n        #[cfg(unix)]\n        Input::Two => {\n            prepare_fallback();\n            finish();\n        }\n    }\n}";

        verify_eq!(
            crate::apply_ast_tree_fix(source, check_match_layout, fix_match_layout),
            expected
        )
    }

    #[gtest]
    fn guards_and_nested_matches_are_formatted_independently() -> Result<()> {
        let source = "fn value(input: Input, nested: Nested) -> usize {\n    match input {\n        Input::One if ready() => match nested {\n            Nested::One => 1,\n            Nested::Two => 2,\n        },\n        Input::Two => match nested {\n            Nested::One => 3,\n            Nested::Two => 4,\n        },\n    }\n}";

        verify_eq!(run(source).len(), 1)
    }

    #[gtest]
    fn compact_matches_stay_compact_regardless_of_arm_count() -> Result<()> {
        let source = EXAMPLES[1].code;

        verify_true!(run(source).is_empty())
    }

    #[gtest]
    fn fix_removes_gaps_from_one_line_arms() -> Result<()> {
        let source = EXAMPLES[2].code;
        let expected = "fn value(input: Input) -> usize {\n    match input {\n        Input::One => 1,\n        Input::Two => 2,\n        Input::Three => 3,\n    }\n}";

        verify_eq!(
            crate::apply_ast_tree_fix(source, check_match_layout, fix_match_layout),
            expected
        )
    }

    #[gtest]
    fn cfg_attribute_does_not_make_compact_arms_spacious() -> Result<()> {
        let source = "fn label(value: Action) -> &'static str {\n    match value {\n        Action::Create => \"create\",\n\n        #[cfg(test)]\n        Action::Inspect => \"inspect\",\n\n        Action::Write => \"write\",\n    }\n}";
        let expected = "fn label(value: Action) -> &'static str {\n    match value {\n        Action::Create => \"create\",\n        #[cfg(test)]\n        Action::Inspect => \"inspect\",\n        Action::Write => \"write\",\n    }\n}";

        verify_eq!(
            crate::apply_ast_tree_fix(source, check_match_layout, fix_match_layout),
            expected
        )
    }

    #[gtest]
    fn empty_arm_diagnostic_does_not_consume_documentation() -> Result<()> {
        let source = "fn visit(value: Value) {\n    match value {\n        Value::Empty => {\n        },\n        Value::Documented => {\n            // Deliberately ignored.\n        },\n    }\n}";

        verify_eq!(run(source).len(), 1)
    }

    #[gtest]
    fn fix_collapses_only_truly_empty_arm_blocks() -> Result<()> {
        let source = "fn visit(value: Value) {\n    match value {\n        Value::Empty => {\n        },\n        Value::Documented => {\n            // Deliberately ignored.\n        },\n    }\n}";
        let expected = "fn visit(value: Value) {\n    match value {\n        Value::Empty => {},\n\n        Value::Documented => {\n            // Deliberately ignored.\n        },\n    }\n}";
        let fixed = crate::apply_ast_tree_fix(source, check_match_layout, fix_match_layout);

        verify_eq!(fixed, expected)?;

        verify_true!(run(&fixed).is_empty())
    }
});
