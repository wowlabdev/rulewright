use ra_ap_syntax::ast::{self, HasGenericArgs as _};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "collect then iter",
        code: "fn f() { (0..10).collect::<Vec<i32>>().iter().count(); }",
        pass: false,
    },
    Example {
        label: "collect then into_iter",
        code: "fn f() { (0..10).collect::<Vec<i32>>().into_iter().count(); }",
        pass: false,
    },
    Example {
        label: "set collect changes semantics",
        code: "fn f() { (0..10).collect::<HashSet<i32>>().into_iter().collect::<Vec<_>>(); }",
        pass: true,
    },
    Example {
        label: "separate collect",
        code: "fn f() { let v: Vec<i32> = (0..10).collect(); v.iter().count(); }",
        pass: true,
    },
    Example {
        label: "collect without iter",
        code: "fn f() { let v: Vec<i32> = (0..10).collect(); }",
        pass: true,
    },
    Example {
        label: "collect iter in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn f() { (0..10).collect::<Vec<i32>>().iter().count(); }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    unnecessary_collect,
    "Flag `.collect().iter()` — remove the intermediate collection.",
    "Collecting into a Vec for one immediate pass usually wastes an allocation. Chain the iterators when evaluation order and borrowing remain correct; keep the collection when it establishes an ownership boundary, permits mutation, or is reused.",
);

fn check_unnecessary_collect(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::MethodCallExpr>()
        .filter(|call| !ctx.is_in_test(call))
        .filter_map(|call| {
            let method = call.name_ref()?.text().to_string();

            if method != "iter" && method != "into_iter" {
                return None;
            }

            let ast::Expr::MethodCallExpr(receiver) = call.receiver()? else {
                return None;
            };

            let collection = receiver.generic_arg_list()?.to_string().replace(' ', "");

            if !collection.starts_with("::<Vec<") {
                return None;
            }

            receiver
                .name_ref()
                .is_some_and(|name| name.text() == "collect")
                .then(|| {
                    ctx.violation(
                        &call,
                        format!(
                            ".collect().{method}() is redundant — remove the intermediate collect"
                        ),
                    )
                })
        })
        .collect()
}

crate::rulewright_ast_test!(check_unnecessary_collect, {
    crate::example_tests!(EXAMPLES, check_unnecessary_collect);
});
