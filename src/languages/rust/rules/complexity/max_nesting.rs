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
    "Deeply nested code is hard to follow. Use early returns, guard clauses, or extract helper functions.",
    Medium,
    params { threshold: i64 = 5 },
);

fn check_max_nesting(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let max_depth = ctx.file.config.get_usize("rust_max_nesting", &PARAMS[0]);

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

impl NestingDepth {
    fn visit_node(&mut self, node: &SyntaxNode, current: usize) {
        for child in node.children() {
            if child.kind() == SyntaxKind::FN {
                continue;
            }

            if ast::Expr::can_cast(child.kind()) {
                let Some(expression) = ast::Expr::cast(child) else {
                    continue;
                };

                self.visit_expression(&expression, current);
            } else {
                self.visit_node(&child, current);
            }
        }
    }

    // #rw(fn: rust_cyclomatic_complexity) structural dispatch mirrors the five nesting constructs
    fn visit_expression(&mut self, expression: &ast::Expr, current: usize) {
        match expression {
            ast::Expr::IfExpr(expression) => {
                if let Some(condition) = expression.condition() {
                    self.visit_expression(&condition, current);
                }

                if let Some(branch) = expression.then_branch() {
                    self.visit_nested(branch.syntax(), current);
                }

                if let Some(branch) = expression.else_branch() {
                    match branch {
                        ast::ElseBranch::Block(branch) => self.visit_node(branch.syntax(), current),
                        ast::ElseBranch::IfExpr(branch) => {
                            self.visit_expression(&ast::Expr::IfExpr(branch), current);
                        }
                    }
                }
            }
            ast::Expr::MatchExpr(expression) => {
                if let Some(scrutinee) = expression.expr() {
                    self.visit_expression(&scrutinee, current);
                }

                let nested = current + 1;

                self.max = self.max.max(nested);

                if let Some(arms) = expression.match_arm_list() {
                    for arm in arms.arms() {
                        if let Some(condition) = arm.guard().and_then(|guard| guard.condition()) {
                            self.visit_expression(&condition, nested);
                        }

                        if let Some(body) = arm.expr() {
                            self.visit_expression(&body, nested);
                        }
                    }
                }
            }
            ast::Expr::ForExpr(expression) => {
                if let Some(iterable) = expression.iterable() {
                    self.visit_expression(&iterable, current);
                }

                if let Some(body) = expression.loop_body() {
                    self.visit_nested(body.syntax(), current);
                }
            }
            ast::Expr::WhileExpr(expression) => {
                if let Some(condition) = expression.condition() {
                    self.visit_expression(&condition, current);
                }

                if let Some(body) = expression.loop_body() {
                    self.visit_nested(body.syntax(), current);
                }
            }
            ast::Expr::LoopExpr(expression) => {
                if let Some(body) = expression.loop_body() {
                    self.visit_nested(body.syntax(), current);
                }
            }
            _ => self.visit_node(expression.syntax(), current),
        }
    }

    fn visit_nested(&mut self, body: &SyntaxNode, current: usize) {
        let nested = current + 1;

        self.max = self.max.max(nested);
        self.visit_node(body, nested);
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
});
