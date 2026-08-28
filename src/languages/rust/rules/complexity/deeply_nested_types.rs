use ra_ap_syntax::{ast, ast::HasGenericArgs};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "shallow type",
        code: "fn f(x: Vec<i32>) {}",
        pass: true,
    },
    Example {
        label: "depth 3 passes",
        code: "fn f(x: Vec<Vec<Vec<i32>>>) {}",
        pass: true,
    },
    Example {
        label: "depth 4 fails",
        code: "fn f(x: Vec<Vec<HashMap<String, Vec<i32>>>>) {}",
        pass: false,
    },
    Example {
        label: "struct field with deep nesting",
        code: "struct S { x: Vec<Vec<HashMap<String, Vec<i32>>>> }",
        pass: false,
    },
    Example {
        label: "return type with deep nesting",
        code: "fn f() -> Vec<Vec<HashMap<String, Vec<i32>>>> { todo!() }",
        pass: false,
    },
    Example {
        label: "deep nesting in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn f(x: Vec<Vec<HashMap<String, Vec<i32>>>>) {}\n}",
        pass: true,
    },
    Example {
        label: "reference to depth 4 fails",
        code: "fn f(x: &Vec<Vec<Vec<Vec<i32>>>>) {}",
        pass: false,
    },
    Example {
        label: "reference to depth 3 passes",
        code: "fn f(x: &Vec<Vec<Vec<i32>>>) {}",
        pass: true,
    },
    Example {
        label: "tuple element depth 4 fails",
        code: "fn f(x: (u8, Vec<Vec<Vec<Vec<i32>>>>)) {}",
        pass: false,
    },
    Example {
        label: "array element depth 4 fails",
        code: "fn f(x: [Vec<Vec<Vec<Vec<i32>>>>; 4]) {}",
        pass: false,
    },
    Example {
        label: "slice element depth 4 fails",
        code: "fn f(x: &[Vec<Vec<Vec<Vec<i32>>>>]) {}",
        pass: false,
    },
    Example {
        label: "parenthesized type depth 4 fails",
        code: "fn f(x: (Vec<Vec<Vec<Vec<i32>>>>)) {}",
        pass: false,
    },
];

crate::ast_rule!(
    deeply_nested_types,
    "Flag type annotations with > 3 levels of generic nesting.",
    "Types like HashMap<String, Vec<Option<Arc<T>>>> hide the domain structure. Introduce an alias or value type only where it gives the nested concept a meaningful name, not merely to move the same punctuation elsewhere.",
);

const MAX_DEPTH: usize = 3;

fn check_deeply_nested_types(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let fields = ctx
        .nodes::<ast::RecordField>()
        .filter(|field| !ctx.is_in_test(field))
        .filter_map(|field| field.ty());
    let tuple_fields = ctx
        .nodes::<ast::TupleField>()
        .filter(|field| !ctx.is_in_test(field))
        .filter_map(|field| field.ty());
    let locals = ctx
        .nodes::<ast::LetStmt>()
        .filter(|local| !ctx.is_in_test(local))
        .filter_map(|local| local.ty());
    let signatures = ctx
        .nodes::<ast::Fn>()
        .filter(|function| !ctx.is_in_test(function))
        .flat_map(|function| {
            function
                .param_list()
                .into_iter()
                .flat_map(|parameters| parameters.params())
                .filter_map(|parameter| parameter.ty())
                .chain(function.ret_type().and_then(|return_type| return_type.ty()))
        });

    fields
        .chain(tuple_fields)
        .chain(locals)
        .chain(signatures)
        .filter_map(|ty| check_type_depth(ctx, &ty))
        .collect()
}

fn check_type_depth(ctx: &AstCtx<'_>, ty: &ast::Type) -> Option<Violation> {
    let depth = type_depth(ty);

    (depth > MAX_DEPTH).then(|| {
        ctx.violation(
            ty,
            format!(
                "type has generic nesting depth {depth} (max {MAX_DEPTH}) — extract a type alias",
            ),
        )
    })
}

fn type_depth(ty: &ast::Type) -> usize {
    let mut max_depth = 0;
    let mut pending = vec![(ty.clone(), 0)];

    while let Some((current, depth)) = pending.pop() {
        max_depth = max_depth.max(depth);

        match current {
            ast::Type::PathType(path_type) => {
                let mut current = path_type.path();

                while let Some(path) = current {
                    if let Some(arguments) = path
                        .segment()
                        .and_then(|segment| segment.generic_arg_list())
                    {
                        pending.extend(arguments.generic_args().filter_map(|argument| {
                            if let ast::GenericArg::TypeArg(argument) = argument {
                                argument.ty().map(|inner| (inner, depth + 1))
                            } else {
                                None
                            }
                        }));
                    }

                    current = path.qualifier();
                }
            }

            ast::Type::RefType(reference) => pending.extend(reference.ty().map(|ty| (ty, depth))),

            ast::Type::SliceType(slice) => pending.extend(slice.ty().map(|ty| (ty, depth))),

            ast::Type::ArrayType(array) => pending.extend(array.ty().map(|ty| (ty, depth))),

            ast::Type::TupleType(tuple) => {
                pending.extend(tuple.fields().map(|ty| (ty, depth)));
            }

            ast::Type::ParenType(paren) => pending.extend(paren.ty().map(|ty| (ty, depth))),

            _ => {}
        }
    }

    max_depth
}

crate::rulewright_ast_test!(check_deeply_nested_types, {
    crate::example_tests!(EXAMPLES, check_deeply_nested_types);
});
