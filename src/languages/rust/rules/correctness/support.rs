use ra_ap_syntax::ast::{self, HasGenericArgs};

pub(super) fn weak_type_name(ty: &ast::Type) -> Option<String> {
    match ty {
        ast::Type::RefType(reference) => match reference.ty()? {
            ast::Type::PathType(path)
                if path
                    .path()?
                    .segment()?
                    .name_ref()
                    .is_some_and(|name| name.text() == "str") =>
            {
                Some("&str".to_string())
            }
            _ => None,
        },
        ast::Type::PathType(path) => {
            let segment = path.path()?.segment()?;

            if segment.generic_arg_list().is_some() {
                return None;
            }

            let name = segment.name_ref()?.text().to_string();

            matches!(
                name.as_str(),
                "u8" | "u16"
                    | "u32"
                    | "u64"
                    | "u128"
                    | "usize"
                    | "i8"
                    | "i16"
                    | "i32"
                    | "i64"
                    | "i128"
                    | "isize"
                    | "f32"
                    | "f64"
                    | "bool"
                    | "char"
                    | "String"
            )
            .then_some(name)
        }
        _ => None,
    }
}
