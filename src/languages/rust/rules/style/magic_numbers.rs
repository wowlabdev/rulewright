use ra_ap_syntax::{
    AstNode,
    ast::{self, LiteralKind, UnaryOp},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "magic number in function",
        code: "fn f() { let x = 42; }",
        pass: false,
    },
    Example {
        label: "zero allowed by default",
        code: "fn f() { let x = 0; }",
        pass: true,
    },
    Example {
        label: "one allowed by default",
        code: "fn f() { let x = 1; }",
        pass: true,
    },
    Example {
        label: "power of two is magic",
        code: "fn f() { let x = 256; }",
        pass: false,
    },
    Example {
        label: "small int is magic",
        code: "fn f() { let x = 2; }",
        pass: false,
    },
    Example {
        label: "round number is magic",
        code: "fn f() { let x = 100; }",
        pass: false,
    },
    Example {
        label: "const passes",
        code: "const N: i32 = 42;",
        pass: true,
    },
    Example {
        label: "static passes",
        code: "static N: i32 = 42;",
        pass: true,
    },
    Example {
        label: "enum discriminant passes",
        code: "enum E { A = 42 }",
        pass: true,
    },
    Example {
        label: "float magic number",
        code: "fn f() { let x = 3.14; }",
        pass: false,
    },
    Example {
        label: "float zero allowed by default",
        code: "fn f() { let x = 0.0; }",
        pass: true,
    },
    Example {
        label: "magic number in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn f() { let x = 42; }\n}",
        pass: true,
    },
    Example {
        label: "negative magic number",
        code: "fn f() { let x = -42; }",
        pass: false,
    },
    Example {
        label: "negative one allowed by default",
        code: "fn f() { let x = -1; }",
        pass: true,
    },
    Example {
        label: "underscored literal is magic",
        code: "fn f() { let x = 1_000; }",
        pass: false,
    },
    Example {
        label: "hex literal is magic",
        code: "fn f() { let x = 0xff; }",
        pass: false,
    },
    Example {
        label: "integer match patterns are not expressions",
        code: "fn f(x: i32) { match x { 2 | 42 => {}, _ => {} } }",
        pass: true,
    },
    Example {
        label: "negative integer match pattern is not an expression",
        code: "fn f(x: i32) { match x { -42 => {}, _ => {} } }",
        pass: true,
    },
    Example {
        label: "literal in match arm body remains magic",
        code: "fn f(x: i32) -> i32 { match x { 0 => 42, _ => 1 } }",
        pass: false,
    },
];

crate::ast_rule!(
    magic_numbers,
    "Flag unnamed numeric literals — extract into named constants.",
    "Unnamed numeric literals obscure intent. Named constants make code self-documenting and easier to update.",
    params {
        allowed: [String] = ["0", "1", "0.0", "1.0"]
    },
);

fn check_magic_numbers(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let allowed = ctx
        .file
        .config
        .get_str_array("rust_magic_numbers", &PARAMS[0]);

    ctx.nodes::<ast::Literal>()
        .filter(|literal| {
            !ctx.is_in_test(literal)
                && !is_pattern_literal(literal)
                && !is_in_const_context(literal)
        })
        .filter_map(|literal| match literal.kind() {
            LiteralKind::IntNumber(number) => {
                let digits = number.value().ok()?.to_string();

                if is_negated(&literal) {
                    (!allowed.iter().any(|allowed| allowed == &digits)).then(|| {
                        let prefix = literal
                            .syntax()
                            .parent()
                            .and_then(ast::PrefixExpr::cast)
                            .expect("negated literal has a prefix expression");

                        ctx.violation(
                            &prefix,
                            format!("magic number `-{digits}` — extract into a named constant"),
                        )
                    })
                } else {
                    (!allowed.iter().any(|allowed| allowed == &digits)).then(|| {
                        ctx.violation(
                            &literal,
                            format!("magic number `{digits}` — extract into a named constant"),
                        )
                    })
                }
            }
            LiteralKind::FloatNumber(number) => {
                let digits = number.value_string();

                (!allowed.iter().any(|allowed| allowed == &digits)).then(|| {
                    ctx.violation(
                        &literal,
                        format!("magic number `{digits}` — extract into a named constant"),
                    )
                })
            }
            _ => None,
        })
        .collect()
}

fn is_pattern_literal(literal: &ast::Literal) -> bool {
    literal
        .syntax()
        .parent()
        .is_some_and(|parent| ast::LiteralPat::can_cast(parent.kind()))
}

fn is_negated(literal: &ast::Literal) -> bool {
    literal
        .syntax()
        .parent()
        .and_then(ast::PrefixExpr::cast)
        .is_some_and(|prefix| prefix.op_kind() == Some(UnaryOp::Neg))
}

fn is_in_const_context(literal: &ast::Literal) -> bool {
    literal.syntax().ancestors().any(|ancestor| {
        ast::Static::can_cast(ancestor.kind())
            || ast::Enum::can_cast(ancestor.kind())
            || ast::Const::cast(ancestor).is_some_and(|item| !is_trait_associated_const(&item))
    })
}

fn is_trait_associated_const(item: &ast::Const) -> bool {
    item.syntax()
        .parent()
        .and_then(ast::AssocItemList::cast)
        .and_then(|list| list.syntax().parent())
        .and_then(ast::Trait::cast)
        .is_some()
}

crate::rulewright_ast_test!(check_magic_numbers, {
    crate::example_tests!(EXAMPLES, check_magic_numbers);
});
