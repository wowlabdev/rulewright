use ra_ap_syntax::ast;

use crate::{AstCtx, Example, Violation};

use super::support::type_arg_last_ident;

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "Arc<Box<T>> field",
        code: "struct S { a: Arc<Box<u32>> }",
        pass: false,
    },
    Example {
        label: "Box<Arc<T>> parameter",
        code: "fn f(x: Box<Arc<u32>>) { drop(x); }",
        pass: false,
    },
    Example {
        label: "Rc<Rc<T>> type alias",
        code: "type T = Rc<Rc<u32>>;",
        pass: false,
    },
    Example {
        label: "Arc<Vec<T>> suggests Arc<[T]>",
        code: "struct S { d: Arc<Vec<u8>> }",
        pass: false,
    },
    Example {
        label: "Arc<String> suggests Arc<str>",
        code: "struct S { n: Arc<String> }",
        pass: false,
    },
    Example {
        label: "Arc<Mutex<T>> is a single heap layer",
        code: "struct S { m: Arc<std::sync::Mutex<u32>> }",
        pass: true,
    },
    Example {
        label: "Box<dyn Trait> is fine",
        code: "struct S { cb: Box<dyn Fn()> }",
        pass: true,
    },
    Example {
        label: "Arc<[T]> and Arc<str> are the goal state",
        code: "fn f(b: Arc<[u8]>, s: Arc<str>) { drop(b); drop(s); }",
        pass: true,
    },
    Example {
        label: "Box<Vec<T>> is owned by rust_box_vec, not this rule",
        code: "fn f() { let _x: Box<Vec<u32>> = Box::new(Vec::new()); }",
        pass: true,
    },
    Example {
        label: "nested pointers in test module",
        code: "#[cfg(test)]\nmod tests {\n    struct S { a: Arc<Box<u32>> }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    nested_smart_pointers,
    "Flag directly nested heap pointers (`Arc<Box<T>>`, `Rc<Rc<T>>`, ...) plus `Arc<Vec<T>>`/`Arc<String>`.",
    "Each nesting layer is another sequential DRAM lookup on access — flatten to one allocation (Arc<[T]>, Arc<str>).",
    Medium,
);

fn check_nested_smart_pointers(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::PathType>()
        .filter(|path| !ctx.is_in_test(path))
        .filter_map(|path_type| {
            let path = path_type.path()?;

            nested_message(&path).map(|message| ctx.violation(&path_type, message))
        })
        .collect()
}

const HEAP_WRAPPERS: &[&str] = &["Arc", "Box", "Rc"];

fn nested_message(path: &ast::Path) -> Option<String> {
    let outer = path.segment()?;
    let outer_name = outer.name_ref()?.text().to_string();

    if !HEAP_WRAPPERS.contains(&outer_name.as_str()) {
        return None;
    }

    let inner = type_arg_last_ident(&outer)?;

    match (outer_name.as_str(), inner.as_str()) {
        ("Box", "Box" | "String" | "Vec") => None,

        (_, "Arc" | "Box" | "Rc") => Some(format!(
            "`{outer_name}<{inner}<..>>` stacks two heap indirections — flatten to a single allocation"
        )),

        ("Arc", "Vec") => Some(
            "`Arc<Vec<T>>` — prefer `Arc<[T]>` (one indirection less, no capacity word)"
                .to_string(),
        ),

        ("Arc", "String") => Some(
            "`Arc<String>` — prefer `Arc<str>` (one indirection less, no capacity word)"
                .to_string(),
        ),

        _ => None,
    }
}

crate::rulewright_ast_test!(check_nested_smart_pointers, {
    crate::example_tests!(EXAMPLES, check_nested_smart_pointers);
});
