use ra_ap_syntax::ast::{self, LiteralKind};

pub(super) use super::super::support::{
    estimate_type_size, parse_int_expr, type_name as self_type_name,
};

const ONE_BYTE: u64 = 1;
const TWO_BYTES: u64 = 2;
const FOUR_BYTES: u64 = 4;
const EIGHT_BYTES: u64 = 8;
const SIXTEEN_BYTES: u64 = 16;

pub(super) fn estimate_fields_size(fields: Option<ast::FieldList>) -> Option<u64> {
    match fields {
        None => Some(0),

        Some(ast::FieldList::RecordFieldList(fields)) => {
            fields.fields().try_fold(0u64, |total, field| {
                Some(total.saturating_add(estimate_type_size(&field.ty()?)?))
            })
        }

        Some(ast::FieldList::TupleFieldList(fields)) => {
            fields.fields().try_fold(0u64, |total, field| {
                Some(total.saturating_add(estimate_type_size(&field.ty()?)?))
            })
        }
    }
}

pub(super) fn literal_elem_size(expr: &ast::Expr) -> Option<u64> {
    let ast::Expr::Literal(literal) = expr else {
        return None;
    };

    match literal.kind() {
        LiteralKind::IntNumber(number) => size_for_suffix(number.suffix()?),
        LiteralKind::FloatNumber(number) => size_for_suffix(number.suffix()?),
        _ => None,
    }
}

fn size_for_suffix(suffix: &str) -> Option<u64> {
    match suffix {
        "u8" | "i8" => Some(ONE_BYTE),
        "u16" | "i16" => Some(TWO_BYTES),
        "u32" | "i32" | "f32" => Some(FOUR_BYTES),
        "u64" | "i64" | "usize" | "isize" | "f64" => Some(EIGHT_BYTES),
        "u128" | "i128" => Some(SIXTEEN_BYTES),
        _ => None,
    }
}
