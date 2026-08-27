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
        code: "fn f(slots: &[Slot]) { let _ = slots.iter().enumerate().filter_map(|(index, slot)| ready(slot).then_some((index, slot))).min_by(|left, right| compare(left, right)).map(first).or_else(|| depleted(slots)); }",
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
    "Flag method-call chains containing at least the configured number of inline closure arguments.",
    "Closure-dense fluent chains hide several branching decisions in one expression. Name an intermediate result or extract a helper.",
    Medium,
    params { threshold: i64 = 3 },
);

fn check_closure_dense_method_chain(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let threshold = ctx
        .file
        .config
        .get_usize("rust_closure_dense_method_chain", &PARAMS[0]);

    let outermost_calls = ctx
        .nodes::<ast::MethodCallExpr>()
        .filter(|call| !ctx.is_in_test(call))
        .filter(|call| !is_receiver_of_method_call(call));

    outermost_calls
        .filter_map(|call| {
            let closure_count = method_chain_inline_closures(&call);

            (closure_count >= threshold).then(|| {
                ctx.violation(
                    &call,
                    format!(
                        "method-call chain contains {closure_count} inline closure arguments (threshold {threshold})"
                    ),
                )
            })
        })
        .collect()
}

fn method_chain_inline_closures(call: &ast::MethodCallExpr) -> usize {
    let mut closure_count = inline_closure_arguments(call);
    let mut receiver = call.receiver();

    while let Some(ast::Expr::MethodCallExpr(inner)) = receiver {
        closure_count += inline_closure_arguments(&inner);
        receiver = inner.receiver();
    }

    closure_count
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
            "fn f(slots: &[Slot]) { let _ = slots.iter().enumerate().filter_map(|(index, slot)| ready(slot).then_some((index, slot))).min_by(|left, right| compare(left, right)).map(first).or_else(|| depleted(slots)); }",
        );

        verify_that!(violations, len(eq(1)))?;
        verify_true!(violations[0].message.contains("3 inline closure arguments"))
    }
});
