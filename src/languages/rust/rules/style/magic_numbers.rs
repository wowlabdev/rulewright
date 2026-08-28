use ra_ap_syntax::{
    AstNode,
    ast::{self, BinaryOp, LiteralKind, UnaryOp},
};
use std::collections::BTreeMap;

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "one-off numeric literal does not justify a constant",
        code: "fn f() { let x = 42; }",
        pass: true,
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
        label: "repeated small integer still needs one shared concept",
        code: "fn f() { let values = [2, 2, 2]; }",
        pass: false,
    },
    Example {
        label: "repeated unexplained value",
        code: "fn f() { let a = 42; let b = 42; let c = 42; }",
        pass: false,
    },
    Example {
        label: "two occurrences stay below the default threshold",
        code: "fn f() { let a = 42; let b = 42; }",
        pass: true,
    },
    Example {
        label: "same value in separate functions is not conflated",
        code: "fn first() { let a = 42; let b = 42; } fn second() { let c = 42; let d = 42; }",
        pass: true,
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
        label: "repeated float literal",
        code: "fn f() { let a = 3.14; let b = 3.14; let c = 3.14; }",
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
        label: "repeated negative literal",
        code: "fn f() { let a = -42; let b = -42; let c = -42; }",
        pass: false,
    },
    Example {
        label: "one-off negative one stays below the threshold",
        code: "fn f() { let x = -1; }",
        pass: true,
    },
    Example {
        label: "repeated negative one is not specially exempt",
        code: "fn f() { let a = -1; let b = -1; let c = -1; }",
        pass: false,
    },
    Example {
        label: "equivalent integer spellings share a value",
        code: "fn f() { let a = 255; let b = 0xff; let c = 255; }",
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
        label: "one-off literal in a match arm does not justify a constant",
        code: "fn f(x: i32) -> i32 { match x { 0 => 42, _ => 1 } }",
        pass: true,
    },
    Example {
        label: "sequence window width is API structure",
        code: "fn f(values: &[u8]) { let _ = values.windows(2); let _ = values.chunks_exact(2); let _ = values.rchunks(2); }",
        pass: true,
    },
    Example {
        label: "array lengths describe type structure",
        code: "fn f(values: [f64; 2]) -> ([f64; 2], [f64; 2]) { (values, values) }",
        pass: true,
    },
    Example {
        label: "arithmetic coefficients stay readable in their formula",
        code: "fn f(x: f64, y: f64) -> f64 { 0.5 * x + 0.5 * y + 0.5 * (x + y) }",
        pass: true,
    },
    Example {
        label: "equal defaults in unrelated record fields are separate concepts",
        code: "fn f() -> Limits { Limits { nodes: 42, edges: 42, bytes: 42 } }",
        pass: true,
    },
    Example {
        label: "same record field repeated across constructions shares a concept",
        code: "fn f() { let _ = A { limit: 42 }; let _ = A { limit: 42 }; let _ = A { limit: 42 }; }",
        pass: false,
    },
];

crate::ast_rule!(
    magic_numbers,
    "Flag numeric values repeated within one function or module scope.",
    "Repeated unexplained values often represent one concept worth naming. One-off literals are left alone because extracting them commonly adds indirection without meaning. Do not hide literals behind a macro; either name a real shared concept or explain why the repetitions are unrelated.",
    Low,
    default = false,
    params {
        allowed: [String] = ["0", "1", "0.0", "1.0"],
        min_occurrences: i64 = 3
    },
);

fn check_magic_numbers(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let allowed = ctx
        .file
        .config
        .get_str_array("rust_magic_numbers", &MAGIC_NUMBERS_PARAMS[0]);
    let min_occurrences = ctx
        .file
        .config
        .get_usize("rust_magic_numbers", &MAGIC_NUMBERS_PARAMS[1]);
    let mut occurrences: BTreeMap<(usize, String, String), Vec<ast::Literal>> = BTreeMap::new();

    for literal in ctx.nodes::<ast::Literal>().filter(|literal| {
        !ctx.is_in_test(literal)
            && !is_pattern_literal(literal)
            && !is_in_const_context(literal)
            && !is_structural_or_formula_literal(literal)
    }) {
        let Some(value) = numeric_value(&literal) else {
            continue;
        };

        if allowed.iter().any(|allowed| allowed == &value) {
            continue;
        }

        occurrences
            .entry((scope_start(&literal), semantic_slot(&literal), value))
            .or_default()
            .push(literal);
    }

    occurrences
        .into_iter()
        .filter(|(_, literals)| literals.len() >= min_occurrences)
        .filter_map(|((_, _, value), literals)| {
            let count = literals.len();
            let first = literals.first()?;
            let message = format!(
                "numeric value `{value}` appears {count} times in one scope — name it only when the occurrences represent one concept; do not hide the literals behind a macro"
            );

            if is_negated(first) {
                let prefix = first.syntax().parent().and_then(ast::PrefixExpr::cast)?;

                Some(ctx.violation(&prefix, message))
            } else {
                Some(ctx.violation(first, message))
            }
        })
        .collect()
}

fn semantic_slot(literal: &ast::Literal) -> String {
    if let Some(label) = literal
        .syntax()
        .ancestors()
        .find_map(ast::ArgList::cast)
        .and_then(|arguments| {
            arguments
                .syntax()
                .descendants()
                .filter_map(ast::Literal::cast)
                .find(|candidate| matches!(candidate.kind(), LiteralKind::String(_)))
        })
    {
        return format!("call-label:{}", label.syntax().text());
    }

    literal
        .syntax()
        .ancestors()
        .find_map(ast::RecordExprField::cast)
        .and_then(|field| field.name_ref())
        .map_or_else(String::new, |name| format!("field:{name}"))
}

fn is_structural_or_formula_literal(literal: &ast::Literal) -> bool {
    literal.syntax().ancestors().skip(1).any(|ancestor| {
        if ast::ArrayType::can_cast(ancestor.kind()) || ast::IndexExpr::can_cast(ancestor.kind()) {
            return true;
        }

        if ast::BinExpr::cast(ancestor.clone())
            .is_some_and(|binary| matches!(binary.op_kind(), Some(BinaryOp::ArithOp(_))))
        {
            return true;
        }

        ast::MethodCallExpr::cast(ancestor)
            .and_then(|call| call.name_ref())
            .is_some_and(|name| {
                matches!(
                    name.text().as_str(),
                    "windows"
                        | "chunks"
                        | "chunks_exact"
                        | "rchunks"
                        | "rchunks_exact"
                        | "array_chunks"
                )
            })
    })
}

fn numeric_value(literal: &ast::Literal) -> Option<String> {
    let value = match literal.kind() {
        LiteralKind::IntNumber(number) => number.value().ok()?.to_string(),
        LiteralKind::FloatNumber(number) => number.value_string(),
        _ => return None,
    };

    Some(if is_negated(literal) {
        format!("-{value}")
    } else {
        value
    })
}

fn scope_start(literal: &ast::Literal) -> usize {
    literal
        .syntax()
        .ancestors()
        .find_map(ast::Fn::cast)
        .map_or(0, |function| function.syntax().text_range().start().into())
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
