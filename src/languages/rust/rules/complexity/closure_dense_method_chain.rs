#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasArgList},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "twenty uniform fluent calls",
        code: "fn f(w: Writer) { let _ = w.push_bind(1).push_bind(2).push_bind(3).push_bind(4).push_bind(5).push_bind(6).push_bind(7).push_bind(8).push_bind(9).push_bind(10).push_bind(11).push_bind(12).push_bind(13).push_bind(14).push_bind(15).push_bind(16).push_bind(17).push_bind(18).push_bind(19).push_bind(20); }",
        pass: true,
    },
    Example {
        label: "long debug builder field list",
        code: "fn f(value: &Value, out: &mut Formatter) { out.debug_struct(\"Value\").field(\"a\", &value.a).field(\"b\", &value.b).field(\"c\", &value.c).field(\"d\", &value.d).field(\"e\", &value.e).field(\"f\", &value.f).field(\"g\", &value.g).field(\"h\", &value.h).field(\"i\", &value.i).field(\"j\", &value.j).finish(); }",
        pass: true,
    },
    Example {
        label: "long closure-free heterogeneous builder",
        code: "fn f(builder: Builder) { let _ = builder.name(1).cost(2).optional(true).retries(3).format(4).target(5).finish(); }",
        pass: true,
    },
    Example {
        label: "one inline closure",
        code: "fn f(xs: &[i32]) { let _ = xs.iter().copied().map(|x| x + 1).collect::<Vec<_>>(); }",
        pass: true,
    },
    Example {
        label: "two inline closures",
        code: "fn f(xs: &[i32]) { let _ = xs.iter().filter(|x| **x > 0).map(|x| x + 1).collect::<Vec<_>>(); }",
        pass: true,
    },
    Example {
        label: "dense selection chain",
        code: "fn f(slots: &[Slot]) { let _ = slots.iter().enumerate().filter_map(|(index, slot)| ready(slot).then_some((index, slot))).filter(|(_, slot)| allowed(slot)).inspect(|item| observe(item)).min_by(|left, right| compare(left, right)).map(|item| first(item)).or_else(|| depleted(slots)); }",
        pass: false,
    },
    Example {
        label: "closure nested in a closure body is not an extra chain closure",
        code: "fn f(xs: Xs) { let _ = xs.map(|| ys.iter().filter(|y| ready(y))).inspect(noop).collect(); }",
        pass: true,
    },
    Example {
        label: "closure in a nested method-call argument belongs to that nested chain",
        code: "fn f(xs: Xs) { let _ = xs.consume(factory.items().filter(|x| ready(x))).map(|x| value(x)); }",
        pass: true,
    },
    Example {
        label: "closure in an unrelated base call is not counted",
        code: "fn f() { let _ = make(|| 1).iter().filter(|x| ready(x)).map(|x| value(x)); }",
        pass: true,
    },
];

crate::ast_rule!(
    closure_dense_method_chain,
    "Flag method-call chains that are very long or contain many inline closure arguments.",
    "Long fluent chains can hide intermediate states, while closure-dense chains can hide several branching decisions in one expression. Split only when the work has meaningful logical stages, bind each stage including the final result, and return the final binding. When one chain already represents a single coherent operation, suppress the finding with that reason instead of inventing arbitrary intermediate variables.",
    Medium,
    params {
        closure_threshold: i64 = 6,
        chain_threshold: i64 = 12
    },
);

fn check_closure_dense_method_chain(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let closure_threshold = ctx.file.config.get_usize(
        "rust_closure_dense_method_chain",
        &CLOSURE_DENSE_METHOD_CHAIN_PARAMS[0],
    );
    let chain_threshold = ctx.file.config.get_usize(
        "rust_closure_dense_method_chain",
        &CLOSURE_DENSE_METHOD_CHAIN_PARAMS[1],
    );

    let outermost_calls = ctx
        .nodes::<ast::MethodCallExpr>()
        .filter(|call| !ctx.is_in_test(call))
        .filter(|call| !is_receiver_of_method_call(call));

    outermost_calls
        .filter_map(|call| {
            let (chain_length, closure_count) = method_chain_metrics(&call);

            if closure_count >= closure_threshold {
                return Some(ctx.violation(
                    &call,
                    format!(
                        "method-call chain contains {closure_count} inline closure arguments (threshold {closure_threshold}, {chain_length} calls total); split meaningful logical stages, or suppress with a reason if this is one coherent operation"
                    ),
                ));
            }

            (chain_length >= chain_threshold && !is_repetitive_fluent_chain(&call)).then(|| {
                ctx.violation(
                    &call,
                    format!(
                        "method-call chain contains {chain_length} calls (threshold {chain_threshold}, {closure_count} inline closure arguments); split meaningful logical stages, or suppress with a reason if this is one coherent operation"
                    ),
                )
            })
        })
        .collect()
}

fn is_repetitive_fluent_chain(call: &ast::MethodCallExpr) -> bool {
    let mut names = Vec::new();
    let mut current = Some(call.clone());

    while let Some(method) = current {
        if let Some(name) = method.name_ref() {
            names.push(name.text().to_string());
        }

        current = match method.receiver() {
            Some(ast::Expr::MethodCallExpr(receiver)) => Some(receiver),
            _ => None,
        };
    }

    if names.len() < 3 {
        return false;
    }

    let without_source = &names[..names.len() - 1];
    let without_terminal_or_source = &names[1..names.len() - 1];

    all_same(without_source) || all_same(without_terminal_or_source)
}

fn all_same(names: &[String]) -> bool {
    names
        .first()
        .is_some_and(|first| names.iter().all(|name| name == first))
}

fn method_chain_metrics(call: &ast::MethodCallExpr) -> (usize, usize) {
    let mut chain_length = 1;
    let mut closure_count = inline_closure_arguments(call);
    let mut receiver = call.receiver();

    while let Some(ast::Expr::MethodCallExpr(inner)) = receiver {
        chain_length += 1;
        closure_count += inline_closure_arguments(&inner);
        receiver = inner.receiver();
    }

    (chain_length, closure_count)
}

fn inline_closure_arguments(call: &ast::MethodCallExpr) -> usize {
    let Some(arguments) = call.arg_list() else {
        return 0;
    };
    let mut closure_count = 0;

    for argument in arguments.args() {
        for node in argument.syntax().descendants() {
            let Some(closure) = ast::ClosureExpr::cast(node) else {
                continue;
            };

            if closure_belongs_to_argument(&closure, call) {
                closure_count += 1;
            }
        }
    }

    closure_count
}

fn closure_belongs_to_argument(closure: &ast::ClosureExpr, call: &ast::MethodCallExpr) -> bool {
    for ancestor in closure.syntax().ancestors().skip(1) {
        if &ancestor == call.syntax() {
            return true;
        }

        if ast::ClosureExpr::can_cast(ancestor.kind())
            || ast::MethodCallExpr::can_cast(ancestor.kind())
        {
            return false;
        }
    }

    false
}

fn is_receiver_of_method_call(call: &ast::MethodCallExpr) -> bool {
    let Some(parent) = call.syntax().parent().and_then(ast::MethodCallExpr::cast) else {
        return false;
    };

    parent
        .receiver()
        .is_some_and(|receiver| receiver.syntax() == call.syntax())
}

crate::rulewright_ast_test!(check_closure_dense_method_chain, {
    crate::example_tests!(EXAMPLES, check_closure_dense_method_chain);

    #[gtest]
    fn reports_only_the_outermost_call() -> Result<()> {
        let violations = run(
            "fn f(slots: &[Slot]) { let _ = slots.iter().enumerate().filter_map(|(index, slot)| ready(slot).then_some((index, slot))).filter(|(_, slot)| allowed(slot)).inspect(|item| observe(item)).min_by(|left, right| compare(left, right)).map(|item| first(item)).or_else(|| depleted(slots)); }",
        );

        verify_that!(violations, len(eq(1)))?;
        verify_true!(violations[0].message.contains("6 inline closure arguments"))?;
        verify_true!(violations[0].message.contains("one coherent operation"))
    }

    #[gtest]
    fn reports_long_chain_without_closures() -> Result<()> {
        let violations =
            run("fn f(w: Writer) { let _ = w.a().b().c().d().e().f().g().h().i().j().k().l(); }");

        verify_that!(violations, len(eq(1)))?;
        verify_true!(violations[0].message.contains("12 calls"))?;
        verify_true!(violations[0].message.contains("0 inline closure arguments"))?;
        verify_true!(violations[0].message.contains("suppress with a reason"))
    }

    #[gtest]
    fn nested_argument_chain_is_measured_separately() -> Result<()> {
        let source = "fn f(xs: Xs) { let _ = xs.consume(factory.a().b().c().d().e().f().g().h().i().j().k().l()).map(value); }";
        let violations = run(source);

        verify_that!(violations, len(eq(1)))?;
        verify_true!(violations[0].message.contains("12 calls"))
    }
});
