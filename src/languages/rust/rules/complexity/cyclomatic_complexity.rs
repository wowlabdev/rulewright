#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "simple function",
        code: "fn f() { let x = 1; }",
        pass: true,
    },
    Example {
        label: "moderate branching is fine",
        code: "fn f(x: bool) { if x {} if x {} if x {} if x {} if x {} if x {} }",
        pass: true,
    },
    Example {
        label: "too many branches",
        code: "fn f(x: bool) { if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} }",
        pass: false,
    },
    Example {
        label: "mixed decision points over threshold",
        code: "fn f(x: u8) -> u8 {\n    match x { 0 => { if a { 1 } else { 2 } }, 1 => { if b { 3 } else { 4 } }, 2 => 5, 3 => 6, _ => 7 }\n    for _ in it { }\n    while a && b { }\n    loop { break; }\n    if p && q { return x; }\n    if r { return 1; }\n    if s { return 2; }\n    if t { return 3; }\n    if u { return 4; }\n    if v { return 5; }\n    x\n}",
        pass: false,
    },
    Example {
        label: "large exhaustive value mapping",
        code: "fn name(value: Value) -> &'static str { match value { Value::A => \"a\", Value::B => \"b\", Value::C => \"c\", Value::D => \"d\", Value::E => \"e\", Value::F => \"f\", Value::G => \"g\", Value::H => \"h\", Value::I => \"i\", Value::J => \"j\", Value::K => \"k\", Value::L => \"l\", Value::M => \"m\", Value::N => \"n\", Value::O => \"o\", Value::P => \"p\" } }",
        pass: true,
    },
    Example {
        label: "flat equality predicate",
        code: "fn same(left: &State, right: &State) -> bool { left.a == right.a && left.b == right.b && left.c == right.c && left.d == right.d && left.e == right.e && left.f == right.f && left.g == right.g && left.h == right.h && left.i == right.i && left.j == right.j && left.k == right.k && left.l == right.l && left.m == right.m && left.n == right.n && left.o == right.o && left.p == right.p }",
        pass: true,
    },
    Example {
        label: "linear error propagation is not branching",
        code: "fn f() -> Result<(), Error> {\n    let a = first()?;\n    let b = second(a)?;\n    let c = third(b)?;\n    fourth(c)?;\n    fifth()?;\n    sixth()?;\n    seventh()?;\n    eighth()?;\n    ninth()?;\n    tenth()?;\n    eleventh()?;\n    twelfth()?;\n    thirteenth()?;\n    fourteenth()?;\n    fifteenth()?;\n    sixteenth()?;\n    Ok(())\n}",
        pass: true,
    },
];

crate::ast_rule!(
    cyclomatic_complexity,
    "Flag functions with structural control-flow complexity above the configured threshold.",
    "Many independently branching execution paths make a function hard to test and prone to bugs; flat predicates and exhaustive value mappings stay readable without artificial helper extraction.",
    Medium,
    params {
        threshold: i64 = 15
    },
);

fn check_cyclomatic_complexity(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let threshold = ctx.file.config.get_usize(
        "rust_cyclomatic_complexity",
        &CYCLOMATIC_COMPLEXITY_PARAMS[0],
    );

    ctx.nodes::<ast::Fn>()
        .filter(|function| !ctx.is_in_test(function))
        .filter_map(|function| {
            function.body()?;
            let complexity = 1 + function
                .syntax()
                .descendants()
                .filter_map(ast::Expr::cast)
                .filter(|expression| enclosing_function(expression).as_ref() == Some(&function))
                .filter(|expression| !is_inside_closure(expression, &function))
                .map(expression_complexity)
                .sum::<usize>();

            (complexity > threshold).then(|| {
                let name = function.name()?;

                Some(ctx.violation(
                    &name,
                    format!(
                        "function `{name}` has cyclomatic complexity {complexity} (max {threshold})"
                    ),
                ))
            })?
        })
        .collect()
}

fn enclosing_function<N>(node: &N) -> Option<ast::Fn>
where
    N: AstNode,
{
    node.syntax().ancestors().skip(1).find_map(ast::Fn::cast)
}

fn is_inside_closure(expression: &ast::Expr, function: &ast::Fn) -> bool {
    expression
        .syntax()
        .ancestors()
        .skip(1)
        .take_while(|ancestor| ancestor != function.syntax())
        .any(|ancestor| ast::ClosureExpr::can_cast(ancestor.kind()))
}

fn expression_complexity(expression: ast::Expr) -> usize {
    match expression {
        ast::Expr::IfExpr(_)
        | ast::Expr::ForExpr(_)
        | ast::Expr::WhileExpr(_)
        | ast::Expr::LoopExpr(_) => 1,
        ast::Expr::MatchExpr(expression) => match_complexity(&expression),
        _ => 0,
    }
}

fn match_complexity(expression: &ast::MatchExpr) -> usize {
    let Some(list) = expression.match_arm_list() else {
        return 0;
    };
    let arms: Vec<ast::MatchArm> = list.arms().collect();
    let guards = arms.iter().filter(|arm| arm.guard().is_some()).count();

    if guards == 0 && arms.iter().all(simple_mapping_arm) {
        usize::from(arms.len() > 1)
    } else {
        arms.len().saturating_sub(1) + guards
    }
}

fn simple_mapping_arm(arm: &ast::MatchArm) -> bool {
    arm.expr().is_some_and(|expression| {
        !matches!(expression, ast::Expr::BlockExpr(_))
            && !expression.syntax().descendants().any(|node| {
                ast::IfExpr::can_cast(node.kind())
                    || ast::MatchExpr::can_cast(node.kind())
                    || ast::ForExpr::can_cast(node.kind())
                    || ast::WhileExpr::can_cast(node.kind())
                    || ast::LoopExpr::can_cast(node.kind())
            })
    })
}

crate::rulewright_ast_test!(check_cyclomatic_complexity, {
    crate::example_tests!(EXAMPLES, check_cyclomatic_complexity);

    #[gtest]
    fn complex_fn_fails() -> Result<()> {
        let src = "fn f(x: bool) {
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
        }";
        let v = run(src);
        verify_eq!(v.len(), 1)?;
        verify_true!(v[0].message.contains("cyclomatic complexity 17"))?;

        Ok(())
    }

    #[gtest]
    fn one_over_threshold_fails() -> Result<()> {
        let src = "fn f(x: bool) {
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
        }";
        let violations = run(src);
        verify_eq!(violations.len(), 1)?;
        verify_true!(violations[0].message.contains("cyclomatic complexity 16"))?;

        Ok(())
    }

    #[gtest]
    fn fourteen_decisions_plus_entry_passes_at_fifteen() -> Result<()> {
        let source = "fn f(x: bool) {
            if x { } if x { } if x { } if x { } if x { }
            if x { } if x { } if x { } if x { } if x { }
            if x { } if x { } if x { } if x { }
        }";
        verify_true!(run(source).is_empty())?;

        Ok(())
    }

    #[gtest]
    fn closure_branches_do_not_inflate_the_enclosing_function() -> Result<()> {
        let closures = (0..20)
            .map(|_| "let _callback = |flag: bool| if flag { 1 } else { 0 };")
            .collect::<Vec<_>>()
            .join("\n");
        let source = format!("fn register() {{ {closures} }}");

        verify_true!(run(&source).is_empty())
    }

    #[gtest]
    fn test_code_passes() -> Result<()> {
        let src = "#[cfg(test)]
        mod tests {
            fn f(x: bool) {
                if x { } if x { } if x { } if x { }
                if x { } if x { } if x { } if x { }
                if x { } if x { } if x { } if x { }
                if x { } if x { } if x { } if x { }
            }
        }";
        verify_true!(run(src).is_empty())?;

        Ok(())
    }
});
