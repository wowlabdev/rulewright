#[cfg(test)]
use googletest::prelude::*;
use std::collections::BTreeSet;

use ra_ap_syntax::{
    AstNode, SyntaxKind,
    ast::{self, ArithOp, BinaryOp, HasArgList, HasLoopBody, LiteralKind, UnaryOp},
};

use crate::{AstCtx, Example, Violation};

use super::super::support::compact_syntax;

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "variable index",
        code: "fn f(v: Vec<i32>, i: usize) { let _ = v[i]; }",
        pass: false,
    },
    Example {
        label: "literal index",
        code: "fn f(v: Vec<i32>) { let _ = v[0]; }",
        pass: true,
    },
    Example {
        label: "indexing in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn f(v: Vec<i32>, i: usize) { let _ = v[i]; }\n}",
        pass: true,
    },
    Example {
        label: "with BOUNDS comment",
        code: "fn f(v: Vec<i32>, i: usize) {\n    assert!(i < v.len());\n    // BOUNDS: the assertion above establishes i < v.len().\n    let _ = v[i];\n}",
        pass: true,
    },
    Example {
        label: "generic BOUNDS comment does not document the invariant",
        code: "fn f(v: Vec<i32>, i: usize) {\n    // BOUNDS: checked above.\n    let _ = v[i];\n}",
        pass: false,
    },
    Example {
        label: "detached BOUNDS comment does not document the operation",
        code: "fn f(v: Vec<i32>, i: usize) {\n    // BOUNDS: i is less than v.len().\n\n    let _ = v[i];\n}",
        pass: false,
    },
    Example {
        label: "index from matching length range",
        code: "fn f(values: &[i32]) { for index in 0..values.len() { let _ = values[index]; } }",
        pass: true,
    },
    Example {
        label: "shadowed range binding does not prove the replacement index",
        code: "fn f(values: &[i32], supplied: usize) { for index in 0..values.len() { let index = supplied; let _ = values[index]; } }",
        pass: false,
    },
    Example {
        label: "index from matching enumerate",
        code: "fn f(values: &[i32]) { for (index, _) in values.iter().enumerate() { let _ = values[index]; } }",
        pass: true,
    },
    Example {
        label: "index guarded by matching while condition",
        code: "fn f(values: &[i32], mut index: usize) { while index < values.len() { let _ = values[index]; index += 1; } }",
        pass: true,
    },
    Example {
        label: "successful binary search index",
        code: "fn f(values: &[i32]) { match values.binary_search(&4) { Ok(index) => { let _ = values[index]; }, Err(_) => {} } }",
        pass: true,
    },
    Example {
        label: "parallel collection is not proven by another collection range",
        code: "fn f(left: &[i32], right: &[i32]) { for index in 0..left.len() { let _ = right[index]; } }",
        pass: false,
    },
    Example {
        label: "cyclic offset from matching length range",
        code: "fn f(values: &[i32]) { for index in 0..values.len() { let _ = values[(index + 1) % values.len()]; } }",
        pass: true,
    },
    Example {
        label: "bounded range iterator feeding a closure",
        code: "fn f(values: &[i32]) { let _ = (0..values.len()).find(|index| values[*index] == 4); }",
        pass: true,
    },
    Example {
        label: "loop-level BOUNDS comment documents parallel collections once",
        code: "fn f(left: &[i32], right: &[i32]) {\n    // BOUNDS: left and right have equal lengths, so index from left is valid for right.\n    for (index, _) in left.iter().enumerate() { let _ = right[index]; }\n}",
        pass: true,
    },
    Example {
        label: "block-level BOUNDS comment covers separated related accesses",
        code: "fn f(left: &[i32], right: &[i32], index: usize) {\n    // BOUNDS: index was checked against the equal lengths of left and right.\n    let _ = left[index];\n    observe();\n    let _ = right[index];\n}",
        pass: true,
    },
];

crate::ast_rule!(
    unchecked_indexing,
    "Flag `container[expr]` indexing with non-literal indices.",
    "Indexing with a variable may panic on out-of-bounds. First make the relationship structural with iteration, `zip`, slices, or `.get()`/`.get_mut()`. A concrete `// BOUNDS:` comment is a last resort for an irreducible fixed-domain or performance invariant, not a mechanical way to silence the rule; one scope comment may cover related accesses, and stacked comments must each name the values and indices or ranges they justify.",
    Low,
    default = false,
);

fn check_unchecked_indexing(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::IndexExpr>()
        .filter(|index| !ctx.is_in_test(index))
        .filter_map(|index| {
            let expr = index.index()?;

            if matches!(expr, ast::Expr::Literal(ref literal) if matches!(literal.kind(), LiteralKind::IntNumber(_)))
                || matches!(&expr, ast::Expr::RangeExpr(range) if compact_syntax(range.syntax()) == "..")
                || has_syntactic_bounds_proof(&index)
            {
                return None;
            }

            let line = ctx.line_of(&expr);

            (!has_concrete_bounds_comment(ctx, &index, line)).then(|| {
                ctx.violation(
                    &expr,
                    "unchecked indexing — make bounds structural with iteration, `zip`, slices, or checked access; reserve a concrete `// BOUNDS:` invariant for an irreducible fixed-domain or performance case",
                )
            })
        })
        .collect()
}

fn has_syntactic_bounds_proof(index: &ast::IndexExpr) -> bool {
    let Some(base) = index.base() else {
        return false;
    };
    let Some(index_expr) = index.index() else {
        return false;
    };
    let base = compact_syntax(base.syntax());
    let Some(binding) = controlling_binding(&base, &index_expr) else {
        return false;
    };

    index_expr.syntax().ancestors().any(|ancestor| {
        if let Some(expression) = ast::ForExpr::cast(ancestor.clone()) {
            return for_loop_proves(&expression, &index_expr, &base, &binding);
        }

        if let Some(expression) = ast::WhileExpr::cast(ancestor.clone()) {
            return expression.loop_body().is_some_and(|body| {
                contains(&body, &index_expr) && !binding_is_shadowed(&body, &index_expr, &binding)
            }) && expression
                .condition()
                .is_some_and(|condition| is_exact_bounds_guard(&condition, &base, &binding));
        }

        if let Some(expression) = ast::IfExpr::cast(ancestor.clone()) {
            return expression.then_branch().is_some_and(|branch| {
                contains(&branch, &index_expr)
                    && !binding_is_shadowed(&branch, &index_expr, &binding)
            }) && expression
                .condition()
                .is_some_and(|condition| is_exact_bounds_guard(&condition, &base, &binding));
        }

        if let Some(expression) = ast::ClosureExpr::cast(ancestor.clone()) {
            return closure_source_proves(&expression, &index_expr, &base, &binding);
        }

        ast::MatchExpr::cast(ancestor).is_some_and(|expression| {
            binary_search_proves(&expression, &index_expr, &base, &binding)
        })
    })
}

fn controlling_binding(base: &str, index: &ast::Expr) -> Option<String> {
    let index = unwrap_parentheses(index.clone());

    if let Some(binding) = direct_binding(&index) {
        return Some(binding);
    }

    let ast::Expr::BinExpr(modulo) = index else {
        return None;
    };

    if modulo.op_kind() != Some(BinaryOp::ArithOp(ArithOp::Rem))
        || !modulo
            .rhs()
            .is_some_and(|divisor| is_matching_len(&divisor, base))
    {
        return None;
    }

    let base_names = expression_names_from_syntax(base);
    let candidates = modulo
        .lhs()?
        .syntax()
        .descendants_with_tokens()
        .filter_map(ra_ap_syntax::NodeOrToken::into_token)
        .filter(|token| token.kind() == SyntaxKind::IDENT)
        .map(|token| token.text().to_string())
        .filter(|name| name != "len" && !base_names.contains(name))
        .collect::<BTreeSet<_>>();

    (candidates.len() == 1)
        .then(|| candidates.into_iter().next())
        .flatten()
}

fn unwrap_parentheses(mut expression: ast::Expr) -> ast::Expr {
    while let ast::Expr::ParenExpr(parenthesized) = expression {
        let Some(inner) = parenthesized.expr() else {
            return ast::Expr::ParenExpr(parenthesized);
        };

        expression = inner;
    }

    expression
}

fn direct_binding(expression: &ast::Expr) -> Option<String> {
    let expression = match expression {
        ast::Expr::PrefixExpr(prefix) if prefix.op_kind() == Some(UnaryOp::Deref) => {
            unwrap_parentheses(prefix.expr()?)
        }

        _ => expression.clone(),
    };
    let ast::Expr::PathExpr(_) = expression else {
        return None;
    };
    let binding = compact_syntax(expression.syntax());

    (!binding.is_empty()
        && binding
            .chars()
            .all(|character| character == '_' || character.is_alphanumeric()))
    .then_some(binding)
}

fn contains(container: &impl AstNode, expression: &ast::Expr) -> bool {
    container
        .syntax()
        .text_range()
        .contains_range(expression.syntax().text_range())
}

fn binding_is_shadowed(container: &impl AstNode, index: &ast::Expr, binding: &str) -> bool {
    let index_start = index.syntax().text_range().start();

    container
        .syntax()
        .descendants()
        .filter_map(ast::LetStmt::cast)
        .filter(|statement| statement.syntax().text_range().start() < index_start)
        .filter_map(|statement| statement.pat())
        .any(|pattern| {
            pattern
                .syntax()
                .descendants_with_tokens()
                .filter_map(ra_ap_syntax::NodeOrToken::into_token)
                .any(|token| token.kind() == SyntaxKind::IDENT && token.text() == binding)
        })
}

fn is_matching_len(expression: &ast::Expr, base: &str) -> bool {
    let ast::Expr::MethodCallExpr(call) = unwrap_parentheses(expression.clone()) else {
        return false;
    };

    call.name_ref()
        .is_some_and(|name| compact_syntax(name.syntax()) == "len")
        && call
            .receiver()
            .is_some_and(|receiver| compact_syntax(receiver.syntax()) == base)
        && call
            .arg_list()
            .is_some_and(|arguments| arguments.args().next().is_none())
}

fn is_exact_bounds_guard(condition: &ast::Expr, base: &str, binding: &str) -> bool {
    let ast::Expr::BinExpr(comparison) = unwrap_parentheses(condition.clone()) else {
        return false;
    };

    compact_syntax(comparison.syntax()) == format!("{binding}<{base}.len()")
}

fn for_loop_proves(
    expression: &ast::ForExpr,
    index: &ast::Expr,
    base: &str,
    binding: &str,
) -> bool {
    let contains_index = expression
        .loop_body()
        .is_some_and(|body| contains(&body, index) && !binding_is_shadowed(&body, index, binding));
    let Some(pattern) = expression
        .pat()
        .map(|pattern| compact_syntax(pattern.syntax()))
    else {
        return false;
    };
    let Some(iterable) = expression
        .iterable()
        .map(|iterable| compact_syntax(iterable.syntax()))
    else {
        return false;
    };
    let bounded_range = iterable == format!("0..{base}.len()")
        && (pattern == binding || pattern == format!("mut{binding}"));
    let matching_enumerate = (iterable == format!("{base}.iter().enumerate()")
        || iterable == format!("{base}.iter_mut().enumerate()"))
        && pattern.starts_with(&format!("({binding},"));

    contains_index && (bounded_range || matching_enumerate)
}

fn closure_source_proves(
    expression: &ast::ClosureExpr,
    index: &ast::Expr,
    base: &str,
    binding: &str,
) -> bool {
    let Some(parameters) = expression
        .param_list()
        .map(|parameters| compact_syntax(parameters.syntax()))
    else {
        return false;
    };
    let Some(call) = expression
        .syntax()
        .parent()
        .and_then(|parent| parent.parent())
        .and_then(ast::MethodCallExpr::cast)
    else {
        return false;
    };
    let passes_iterator_items = call
        .name_ref()
        .is_some_and(|name| compact_syntax(name.syntax()) == "find");
    let Some(receiver) = call
        .receiver()
        .map(|receiver| compact_syntax(receiver.syntax()))
    else {
        return false;
    };
    let bounded_range = (receiver == format!("(0..{base}.len())")
        || receiver == format!("0..{base}.len()"))
        && parameters == format!("|{binding}|");
    let matching_enumerate = (receiver == format!("{base}.iter().enumerate()")
        || receiver == format!("{base}.iter_mut().enumerate()"))
        && parameters.starts_with(&format!("|({binding},"));

    passes_iterator_items
        && !binding_is_shadowed(expression, index, binding)
        && (bounded_range || matching_enumerate)
}

fn binary_search_proves(
    expression: &ast::MatchExpr,
    index: &ast::Expr,
    base: &str,
    binding: &str,
) -> bool {
    let matching_search = expression.expr().is_some_and(|scrutinee| {
        let ast::Expr::MethodCallExpr(call) = unwrap_parentheses(scrutinee) else {
            return false;
        };

        call.name_ref()
            .is_some_and(|name| compact_syntax(name.syntax()) == "binary_search")
            && call
                .receiver()
                .is_some_and(|receiver| compact_syntax(receiver.syntax()) == base)
    });
    let successful_arm = index
        .syntax()
        .ancestors()
        .take_while(|ancestor| ancestor != expression.syntax())
        .find_map(ast::MatchArm::cast)
        .is_some_and(|arm| {
            arm.pat()
                .is_some_and(|pattern| compact_syntax(pattern.syntax()) == format!("Ok({binding})"))
                && arm
                    .expr()
                    .is_some_and(|body| !binding_is_shadowed(&body, index, binding))
        });

    matching_search && successful_arm
}

fn expression_names_from_syntax(source: &str) -> BTreeSet<String> {
    source
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

fn has_concrete_bounds_comment(ctx: &AstCtx<'_>, index: &ast::IndexExpr, line: usize) -> bool {
    let Some(comment) = bounds_comment(ctx, index, line) else {
        return false;
    };
    let Some(base) = index.base() else {
        return false;
    };
    let Some(index_expr) = index.index() else {
        return false;
    };
    let base_names = expression_names(&base);
    let index_names = expression_names(&index_expr);

    !base_names.is_empty()
        && base_names
            .iter()
            .any(|name| comment_contains_name(&comment, name))
        && (index_names.is_empty()
            || index_names
                .iter()
                .any(|name| comment_contains_name(&comment, name)))
}

fn bounds_comment(ctx: &AstCtx<'_>, index: &ast::IndexExpr, line: usize) -> Option<String> {
    attached_bounds_comment(ctx.file.lines, line)
        .or_else(|| {
            index.syntax().ancestors().skip(1).find_map(|ancestor| {
                let expression = ast::Expr::cast(ancestor)?;

                matches!(
                    expression,
                    ast::Expr::ForExpr(_)
                        | ast::Expr::WhileExpr(_)
                        | ast::Expr::LoopExpr(_)
                        | ast::Expr::MatchExpr(_)
                )
                .then(|| ctx.line_of(&expression))
                .and_then(|scope_line| attached_bounds_comment(ctx.file.lines, scope_line))
            })
        })
        .or_else(|| {
            index
                .syntax()
                .ancestors()
                .skip(1)
                .filter_map(ast::BlockExpr::cast)
                .find_map(|block| leading_bounds_comment(ctx.file.lines, ctx.line_of(&block)))
        })
}

fn leading_bounds_comment(lines: &[&str], opening_line: usize) -> Option<String> {
    let mut comments = Vec::new();

    for raw in lines.iter().skip(opening_line).take(4) {
        let trimmed = raw.trim();

        if trimmed.is_empty() {
            if comments.is_empty() {
                continue;
            }

            return None;
        }

        let Some(comment) = trimmed.strip_prefix("//") else {
            break;
        };

        comments.push(comment.trim());
    }

    let comment = comments.join(" ");
    let (_, explanation) = comment.split_once("BOUNDS:")?;

    (!explanation.trim().is_empty()).then_some(explanation.trim().to_owned())
}

fn attached_bounds_comment(lines: &[&str], line: usize) -> Option<String> {
    let mut comments = Vec::new();

    for raw in lines.get(..line.saturating_sub(1))?.iter().rev().take(3) {
        let trimmed = raw.trim();
        let Some(comment) = trimmed.strip_prefix("//") else {
            break;
        };

        comments.push(comment.trim());
    }

    comments.reverse();
    let comment = comments.join(" ");
    let (_, explanation) = comment.split_once("BOUNDS:")?;

    (!explanation.trim().is_empty()).then_some(explanation.trim().to_owned())
}

fn expression_names(expr: &ast::Expr) -> BTreeSet<String> {
    expr.syntax()
        .descendants_with_tokens()
        .filter_map(ra_ap_syntax::NodeOrToken::into_token)
        .filter(|token| token.kind() == SyntaxKind::IDENT)
        .map(|token| token.text().to_string())
        .collect()
}

fn comment_contains_name(comment: &str, name: &str) -> bool {
    comment
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .any(|word| word == name)
}

crate::rulewright_ast_test!(check_unchecked_indexing, {
    crate::example_tests!(EXAMPLES, check_unchecked_indexing);

    #[gtest]
    fn bounds_comment_may_name_a_field_container() -> Result<()> {
        let source = "fn f(state: State, index: usize) {\n    // BOUNDS: index was checked against state.points.len().\n    let _ = state.points[index];\n}";

        verify_true!(run(source).is_empty())
    }

    #[gtest]
    fn range_comment_must_name_a_bound() -> Result<()> {
        let source = "fn f(values: &[u8], end: usize) {\n    // BOUNDS: values has enough elements.\n    let _ = &values[..end];\n}";

        verify_that!(run(source), len(eq(1)))
    }

    #[gtest]
    fn separate_bounds_invariants_may_be_stacked() -> Result<()> {
        let source = "fn f(left: &[u8], right: &[u8], left_index: usize, right_index: usize) {\n    // BOUNDS: left_index was checked against left.len().\n    // BOUNDS: right_index was checked against right.len().\n    let _ = (left[left_index], right[right_index]);\n}";

        verify_true!(run(source).is_empty())
    }

    #[gtest]
    fn range_proof_does_not_cover_a_different_index_expression() -> Result<()> {
        let source =
            "fn f(values: &[u8]) { for index in 0..values.len() { let _ = values[index + 1]; } }";

        verify_that!(run(source), len(eq(1)))
    }

    #[gtest]
    fn negated_or_disjunctive_conditions_are_not_bounds_proofs() -> Result<()> {
        let negated = "fn f(values: &[u8], index: usize) { if !(index < values.len()) { let _ = values[index]; } }";
        let disjunctive = "fn f(values: &[u8], index: usize, force: bool) { if index < values.len() || force { let _ = values[index]; } }";

        verify_that!(run(negated), len(eq(1)))?;

        verify_that!(run(disjunctive), len(eq(1)))
    }

    #[gtest]
    fn bounds_guard_does_not_cover_the_else_branch() -> Result<()> {
        let source = "fn f(values: &[u8], index: usize) { if index < values.len() { return; } else { let _ = values[index]; } }";

        verify_that!(run(source), len(eq(1)))
    }

    #[gtest]
    fn binary_search_proof_only_covers_the_success_arm() -> Result<()> {
        let source = "fn f(values: &[u8], index: usize) { match values.binary_search(&4) { Ok(index) => { let _ = values[index]; }, Err(_) => { let _ = values[index]; } } }";

        verify_that!(run(source), len(eq(1)))
    }

    #[gtest]
    fn enumerate_proof_only_uses_the_position_binding() -> Result<()> {
        let loop_source = "fn f(values: &[usize]) { for (_, value) in values.iter().enumerate() { let _ = values[value]; } }";
        let closure_source = "fn f(values: &[usize]) { let _ = values.iter().enumerate().find(|(_, value)| values[*value] == 4); }";

        verify_that!(run(loop_source), len(eq(1)))?;

        verify_that!(run(closure_source), len(eq(1)))
    }
});
