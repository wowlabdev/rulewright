#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{
    AstNode, SyntaxKind, SyntaxNode,
    ast::{self, HasLoopBody, HasName},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "shallow function",
        code: "fn f() { if true { if true { } } }",
        pass: true,
    },
    Example {
        label: "match/loop/else nested to depth 6 fails",
        code: "fn f() {\n    loop {\n        while c {\n            if a {\n            } else {\n                match x {\n                    _ => loop {\n                        match y {\n                            _ => loop { }\n                        }\n                    }\n                }\n            }\n        }\n    }\n}",
        pass: false,
    },
    Example {
        label: "match/loop nested to depth 5 passes",
        code: "fn f() {\n    loop {\n        while c {\n            match x {\n                _ => loop {\n                    match y {\n                        _ => ()\n                    }\n                }\n            }\n        }\n    }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    max_nesting,
    "Flag nesting depth > threshold levels.",
    "Deeply nested code hides the active conditions. Prefer early returns, guard clauses, or a helper for a coherent sub-operation; do not invert straightforward control flow or extract arbitrary blocks solely to lower the measured depth.",
    Medium,
    params { threshold: i64 = 5 },
);

fn check_max_nesting(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let max_depth = ctx
        .file
        .config
        .get_usize("rust_max_nesting", &MAX_NESTING_PARAMS[0]);

    ctx.nodes::<ast::Fn>()
        .filter(|function| !ctx.is_in_test(function))
        .filter_map(|function| {
            let body = function.body()?;
            let mut depth = NestingDepth::default();

            depth.visit_node(body.syntax(), 0);

            (depth.max > max_depth).then(|| {
                let name = function.name()?;

                Some(ctx.violation(
                    &name,
                    format!(
                        "function `{name}` has nesting depth {} (max {max_depth})",
                        depth.max
                    ),
                ))
            })?
        })
        .collect()
}

#[derive(Default)]
struct NestingDepth {
    max: usize,
}

enum PendingVisit {
    Node(SyntaxNode, usize),
    Expression(ast::Expr, usize),
}

fn queue_children(node: &SyntaxNode, current: usize, pending: &mut Vec<PendingVisit>) {
    for child in node.children() {
        if child.kind() == SyntaxKind::FN {
            continue;
        }

        match ast::Expr::cast(child.clone()) {
            Some(expression) => pending.push(PendingVisit::Expression(expression, current)),
            None => pending.push(PendingVisit::Node(child, current)),
        }
    }
}

impl NestingDepth {
    fn visit_node(&mut self, node: &SyntaxNode, current: usize) {
        let mut pending = vec![PendingVisit::Node(node.clone(), current)];

        while let Some(visit) = pending.pop() {
            match visit {
                PendingVisit::Node(node, depth) => queue_children(&node, depth, &mut pending),

                PendingVisit::Expression(expression, depth) => {
                    self.visit_expression(&expression, depth, &mut pending);
                }
            }
        }
    }

    fn visit_expression(
        &mut self,
        expression: &ast::Expr,
        current: usize,
        pending: &mut Vec<PendingVisit>,
    ) {
        match expression {
            ast::Expr::IfExpr(expression) => {
                if let Some(condition) = expression.condition() {
                    pending.push(PendingVisit::Expression(condition, current));
                }

                if let Some(branch) = expression.then_branch() {
                    self.queue_nested(branch.syntax(), current, pending);
                }

                if let Some(branch) = expression.else_branch() {
                    match branch {
                        ast::ElseBranch::Block(branch) => {
                            self.queue_nested(branch.syntax(), current, pending);
                        }

                        ast::ElseBranch::IfExpr(branch) => {
                            pending
                                .push(PendingVisit::Expression(ast::Expr::IfExpr(branch), current));
                        }
                    }
                }
            }

            ast::Expr::MatchExpr(expression) => {
                if let Some(scrutinee) = expression.expr() {
                    pending.push(PendingVisit::Expression(scrutinee, current));
                }

                let nested = current + 1;

                self.max = self.max.max(nested);

                if let Some(arms) = expression.match_arm_list() {
                    for arm in arms.arms() {
                        if let Some(condition) = arm.guard().and_then(|guard| guard.condition()) {
                            pending.push(PendingVisit::Expression(condition, nested));
                        }

                        if let Some(body) = arm.expr() {
                            pending.push(PendingVisit::Expression(body, nested));
                        }
                    }
                }
            }

            ast::Expr::ForExpr(expression) => {
                if let Some(iterable) = expression.iterable() {
                    pending.push(PendingVisit::Expression(iterable, current));
                }

                if let Some(body) = expression.loop_body() {
                    self.queue_nested(body.syntax(), current, pending);
                }
            }

            ast::Expr::WhileExpr(expression) => {
                if let Some(condition) = expression.condition() {
                    pending.push(PendingVisit::Expression(condition, current));
                }

                if let Some(body) = expression.loop_body() {
                    self.queue_nested(body.syntax(), current, pending);
                }
            }

            ast::Expr::LoopExpr(expression) => {
                if let Some(body) = expression.loop_body() {
                    self.queue_nested(body.syntax(), current, pending);
                }
            }

            _ => pending.push(PendingVisit::Node(expression.syntax().clone(), current)),
        }
    }

    fn queue_nested(&mut self, body: &SyntaxNode, current: usize, pending: &mut Vec<PendingVisit>) {
        let nested = current + 1;

        self.max = self.max.max(nested);
        pending.push(PendingVisit::Node(body.clone(), nested));
    }
}

crate::rulewright_ast_test!(check_max_nesting, {
    crate::example_tests!(EXAMPLES, check_max_nesting);

    #[gtest]
    fn deeply_nested_fails() -> Result<()> {
        let src = "fn f() {
            if true {           // 1
                if true {       // 2
                    if true {   // 3
                        if true { // 4
                            if true { // 5
                                if true { } // 6
                            }
                        }
                    }
                }
            }
        }";
        let v = run(src);
        verify_eq!(v.len(), 1)?;
        verify_true!(v[0].message.contains("nesting depth 6"))?;

        Ok(())
    }

    #[gtest]
    fn at_threshold_passes() -> Result<()> {
        let src = "fn f() {
            if true {           // 1
                if true {       // 2
                    if true {   // 3
                        if true { // 4
                            if true { } // 5
                        }
                    }
                }
            }
        }";
        verify_true!(run(src).is_empty())?;

        Ok(())
    }

    #[gtest]
    fn nested_else_blocks_count_toward_the_limit() -> Result<()> {
        let source = "fn f() {
            if a {} else {                 // 1
                if b {} else {             // 2
                    if c {} else {         // 3
                        if d {} else {     // 4
                            if e {} else { // 5
                                if f {}    // 6
                            }
                        }
                    }
                }
            }
        }";
        let violations = run(source);

        verify_eq!(violations.len(), 1)?;

        verify_true!(violations[0].message.contains("nesting depth 6"))
    }

    #[gtest]
    fn else_if_chain_is_one_control_level() -> Result<()> {
        let source =
            "fn f() { if a {} else if b {} else if c {} else if d {} else if e {} else if f {} }";

        verify_true!(run(source).is_empty())
    }
});
