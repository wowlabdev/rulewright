use ra_ap_syntax::{ast, ast::LiteralKind};

use super::super::support::is_in_const_or_static;
use crate::{AstCtx, Example, Fix, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "suffixed literal cast",
        code: "fn f() { let x = 1u8 as u32; }",
        pass: false,
    },
    Example {
        label: "unsuffixed literal",
        code: "fn f() { let x = 1 as u32; }",
        pass: true,
    },
    Example {
        label: "unknown source type",
        code: "fn f(a: u8) { let x = a as u32; }",
        pass: true,
    },
    Example {
        label: "const context",
        code: "const X: u32 = 1u8 as u32;",
        pass: true,
    },
    Example {
        label: "static context",
        code: "static X: u32 = 1u8 as u32;",
        pass: true,
    },
    Example {
        label: "cast in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn f() { let x = 1u8 as u32; }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    from_instead_of_as,
    "Flag `as` casts on suffixed literals — use `From`/`Into` instead.",
    "From/Into conversions are checked at compile time. 'as' casts silently truncate, which hides bugs.",
    Low,
    fix_from_instead_of_as,
);

fn check_from_instead_of_as(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let numeric_casts = ctx
        .nodes::<ast::CastExpr>()
        .filter(|cast| !ctx.is_in_test(cast) && !is_in_const_or_static(cast))
        .filter(|cast| {
            cast.expr().is_some_and(|expr| has_numeric_suffix(&expr))
                && cast.ty().is_some_and(|ty| is_numeric_type(&ty))
        });

    numeric_casts
        .map(|cast| {
            ctx.violation(
                &cast,
                "prefer `From`/`Into` instead of `as` cast on suffixed literal",
            )
        })
        .collect()
}

fn has_numeric_suffix(expr: &ast::Expr) -> bool {
    match expr {
        ast::Expr::Literal(literal) => match literal.kind() {
            LiteralKind::IntNumber(number) => number.suffix().is_some(),
            LiteralKind::FloatNumber(number) => number.suffix().is_some(),
            _ => false,
        },

        _ => false,
    }
}

fn is_numeric_type(ty: &ast::Type) -> bool {
    let ast::Type::PathType(path_type) = ty else {
        return false;
    };

    let name = path_type
        .path()
        .and_then(|path| path.segment())
        .and_then(|segment| segment.name_ref());

    name.is_some_and(|name| NUMERIC_TYPES.contains(&name.text().as_str()))
}

const NUMERIC_TYPES: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "f32",
    "f64",
];

fn fix_from_instead_of_as(ctx: &AstCtx<'_>, v: &Violation) -> Option<Fix> {
    let line = ctx.file.line(v.line)?;
    let (before_as, after_as) = single_cast(line)?;
    let target_type = numeric_prefix(after_as)?;
    let suffix = numeric_suffix(before_as)?;
    let number_part = before_as.strip_suffix(suffix)?;
    let lit_start = literal_start(number_part)?;
    let literal = before_as.get(lit_start..)?;
    let before_literal = line.get(..lit_start)?;
    let after_type = after_as.strip_prefix(target_type)?;

    let fixed = format!("{before_literal}{target_type}::from({literal}){after_type}");

    Some(Fix::replace_line(v.line, fixed))
}

fn single_cast(line: &str) -> Option<(&str, &str)> {
    let (before, after) = line.split_once(" as ")?;

    (!after.contains(" as ")).then_some((before, after))
}

fn numeric_prefix(value: &str) -> Option<&'static str> {
    NUMERIC_TYPES.iter().copied().find(|numeric_type| {
        value.strip_prefix(numeric_type).is_some_and(|rest| {
            rest.chars()
                .next()
                .is_none_or(|character| !character.is_alphanumeric() && character != '_')
        })
    })
}

fn numeric_suffix(value: &str) -> Option<&'static str> {
    NUMERIC_TYPES
        .iter()
        .copied()
        .find(|numeric_type| value.ends_with(numeric_type))
}

fn literal_start(number_part: &str) -> Option<usize> {
    number_part
        .char_indices()
        .rev()
        .take_while(|&(_, c)| {
            c.is_ascii_digit()
                || c == '.'
                || c == '_'
                || c == 'x'
                || c == 'b'
                || c == 'o'
                || c.is_ascii_hexdigit()
        })
        .last()
        .map(|(index, _)| index)
}

crate::rulewright_ast_test!(check_from_instead_of_as, {
    crate::example_tests!(EXAMPLES, check_from_instead_of_as);
    crate::fix_tests!(
        EXAMPLES,
        ast,
        check_from_instead_of_as,
        fix_from_instead_of_as
    );
});
