#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::ast::{self, LiteralKind};
use winnow::token::take_while;

use crate::{AstCtx, Example, Violation, infra::parse};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "f32 with too many digits",
        code: "fn f() { let _x: f32 = 1.23456789012345_f32; }",
        pass: false,
    },
    Example {
        label: "f32 with ok precision",
        code: "fn f() { let _x: f32 = 1.234567_f32; }",
        pass: true,
    },
    Example {
        label: "f64 with too many digits",
        code: "fn f() { let _x = 3.14159265358979323846_f64; }",
        pass: false,
    },
    Example {
        label: "f64 with ok precision",
        code: "fn f() { let _x = 3.141592653589793_f64; }",
        pass: true,
    },
    Example {
        label: "unsuffixed float is fine",
        code: "fn f() { let _x = 3.14159265358979323846; }",
        pass: true,
    },
    Example {
        label: "excessive precision in test",
        code: "#[cfg(test)]\nmod tests {\n    fn t() { let _x = 1.23456789012345_f32; }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    excessive_float_precision,
    "Flag float literals with more significant digits than the type can represent.",
    "Extra digits beyond what f32/f64 can represent are misleading. They suggest precision that does not exist.",
);

/// f32 has ~7 significant decimal digits, f64 has ~16.
const F32_MAX_SIGNIFICANT: usize = 8;
const F64_MAX_SIGNIFICANT: usize = 17;

fn check_excessive_float_precision(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::Literal>()
        .filter(|literal| !ctx.is_in_test(literal))
        .filter_map(|literal| {
            let LiteralKind::FloatNumber(number) = literal.kind() else {
                return None;
            };
            let repr = number.to_string();
            let (max_digits, type_name) = match number.suffix()? {
                "f32" => (F32_MAX_SIGNIFICANT, "f32"),
                "f64" => (F64_MAX_SIGNIFICANT, "f64"),
                _ => return None,
            };
            let sig_digits = count_significant_digits(&repr);

            (sig_digits > max_digits).then(|| {
                ctx.violation(
                    &literal,
                    format!(
                        "float literal has {sig_digits} significant digits but {type_name} only supports ~{} — excess digits are silently lost",
                        max_digits - 1
                    ),
                )
            })
        })
        .collect()
}

fn count_significant_digits(s: &str) -> usize {
    let mut input = s;
    let body = parse::try_parse(&mut input, take_while(0.., |c: char| c != 'f')).unwrap_or(s);
    let cleaned: String = body.chars().filter(|&c| c != '_').collect();

    let mut inp = cleaned.as_str();
    let _ = parse::try_parse(&mut inp, take_while(0.., |c: char| c.is_ascii_digit()));

    if parse::try_parse(&mut inp, '.').is_none() {
        return 0;
    }

    let all_digits: String = cleaned.chars().filter(char::is_ascii_digit).collect();
    let mut digit_input = all_digits.as_str();
    let _ = parse::try_parse(&mut digit_input, take_while(0.., '0'));

    digit_input.len()
}

crate::rulewright_ast_test!(check_excessive_float_precision, {
    crate::example_tests!(EXAMPLES, check_excessive_float_precision);

    #[gtest]
    fn count_digits() -> Result<()> {
        verify_eq!(count_significant_digits("3.14_f32"), 3)?;
        verify_eq!(count_significant_digits("0.001_f32"), 1)?;
        verify_eq!(count_significant_digits("1.234567890_f64"), 10)?;
        verify_eq!(count_significant_digits("100_f32"), 0)?;

        Ok(())
    }
});
