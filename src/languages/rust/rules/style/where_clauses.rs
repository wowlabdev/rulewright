use ra_ap_syntax::{
    AstNode,
    ast::{self, HasTypeBounds},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "function where clause",
        code: "fn render<T>(value: T) where T: std::fmt::Display {}",
        pass: true,
    },
    Example {
        label: "inline function bound",
        code: "fn render<T: std::fmt::Display>(value: T) {}",
        pass: false,
    },
    Example {
        label: "inline struct bound",
        code: "struct Wrapper<T: Clone> { value: T }",
        pass: false,
    },
    Example {
        label: "lifetime bounds are exempt",
        code: "struct Borrowed<'a: 'static> { value: &'a str }",
        pass: true,
    },
    Example {
        label: "const generics are exempt",
        code: "struct Buffer<const N: usize> { bytes: [u8; N] }",
        pass: true,
    },
    Example {
        label: "default type params are exempt",
        code: "struct Wrapper<T: Clone = String> { value: T }",
        pass: true,
    },
];

crate::ast_rule!(
    where_clauses,
    "Require type-parameter trait bounds to use where clauses.",
    "Where clauses keep function and type declarations readable as constraints grow.",
    Low,
);

fn check_where_clauses(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let bounded_params = ctx
        .nodes::<ast::TypeParam>()
        .filter(|param| param.default_type().is_none())
        .filter(|param| {
            param
                .type_bound_list()
                .is_some_and(|bounds| bounds.bounds().next().is_some())
        });

    bounded_params
        .filter(|param| {
            param.syntax().ancestors().any(|ancestor| {
                ast::Fn::can_cast(ancestor.kind())
                    || ast::Impl::can_cast(ancestor.kind())
                    || ast::Struct::can_cast(ancestor.kind())
                    || ast::Enum::can_cast(ancestor.kind())
            })
        })
        .map(|param| {
            ctx.violation(
                &param,
                "trait bounds in generic parameter lists must move to a where clause",
            )
        })
        .collect()
}

crate::rulewright_ast_test!(check_where_clauses, {
    crate::example_tests!(EXAMPLES, check_where_clauses);
});
