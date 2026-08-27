use ra_ap_syntax::ast;

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "pub fn taking Range",
        code: "pub fn f(r: Range<usize>) {}",
        pass: false,
    },
    Example {
        label: "pub fn taking std::ops::Range",
        code: "pub fn f(r: std::ops::Range<usize>) {}",
        pass: false,
    },
    Example {
        label: "pub method taking Range",
        code: "pub struct S;\nimpl S {\n    pub fn f(&self, r: Range<u32>) {}\n}",
        pass: false,
    },
    Example {
        label: "private fn taking Range",
        code: "fn f(r: Range<usize>) {}",
        pass: true,
    },
    Example {
        label: "impl RangeBounds parameter",
        code: "pub fn f(r: impl std::ops::RangeBounds<usize>) {}",
        pass: true,
    },
    Example {
        label: "Range behind a reference",
        code: "pub fn f(r: &Range<usize>) {}",
        pass: true,
    },
    Example {
        label: "RangeInclusive is not flagged",
        code: "pub fn f(r: RangeInclusive<usize>) {}",
        pass: true,
    },
    Example {
        label: "Range in test module",
        code: "#[cfg(test)]\nmod tests {\n    pub fn f(r: Range<usize>) {}\n}",
        pass: true,
    },
];

crate::ast_rule!(
    range_over_rangebounds,
    "Flag `pub` fn parameters typed `Range<T>` — accept `impl RangeBounds<T>` instead.",
    "Range<T> forces callers to supply half-open bounds; impl RangeBounds<T> also accepts `1..`, `..=n`, and `..`.",
    Low,
);

fn check_range_over_rangebounds(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::Fn>()
        .filter(|function| {
            super::support::is_item_or_impl_fn(function)
                && !ctx.is_in_test(function)
                && super::support::is_fully_public(function)
        })
        .flat_map(|function| {
            function
                .param_list()
                .into_iter()
                .flat_map(|params| params.params())
                .filter_map(|param| {
                    let ty = param.ty()?;

                    is_range_path(&ty).then(|| {
                        ctx.violation(
                            &ty,
                            "public fn parameter typed `Range<T>` — accept `impl RangeBounds<T>` for caller flexibility",
                        )
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn is_range_path(ty: &ast::Type) -> bool {
    let ast::Type::PathType(path) = ty else {
        return false;
    };
    let Some(path) = path.path() else {
        return false;
    };

    super::support::path_matches(path.clone(), &["Range"])
        || super::support::path_matches(path, &["std", "ops", "Range"])
}

crate::rulewright_ast_test!(check_range_over_rangebounds, {
    crate::example_tests!(EXAMPLES, check_range_over_rangebounds);
});
