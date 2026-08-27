use ra_ap_syntax::ast;

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "&String parameter",
        code: "fn f(x: &String) {}",
        pass: false,
    },
    Example {
        label: "&PathBuf parameter",
        code: "fn f(x: &PathBuf) {}",
        pass: false,
    },
    Example {
        label: "&Vec parameter",
        code: "fn f(x: &Vec<u32>) {}",
        pass: false,
    },
    Example {
        label: "fully qualified &OsString parameter",
        code: "fn f(x: &std::ffi::OsString) {}",
        pass: false,
    },
    Example {
        label: "&String in impl method",
        code: "struct S;\nimpl S {\n    fn f(&self, x: &String) {}\n}",
        pass: false,
    },
    Example {
        label: "&str parameter",
        code: "fn f(x: &str) {}",
        pass: true,
    },
    Example {
        label: "&mut String needs the owned type",
        code: "fn f(x: &mut String) {}",
        pass: true,
    },
    Example {
        label: "owned String parameter",
        code: "fn f(x: String) {}",
        pass: true,
    },
    Example {
        label: "slice parameter",
        code: "fn f(x: &[u32]) {}",
        pass: true,
    },
    Example {
        label: "&String in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn f(x: &String) {}\n}",
        pass: true,
    },
];

crate::ast_rule!(
    owned_ref_param,
    "Flag fn parameters typed `&String`, `&PathBuf`, `&Vec<T>`, `&OsString`.",
    "A shared reference to an owned container forces callers to materialize the owned type; borrowed forms accept more argument types for free.",
    Medium,
);

const OWNED_REF_SUGGESTIONS: &[(&str, &str)] = &[
    ("String", "`&str` or `impl AsRef<str>`"),
    ("PathBuf", "`&Path` or `impl AsRef<Path>`"),
    ("Vec", "`&[T]` or `impl AsRef<[T]>`"),
    ("OsString", "`&OsStr` or `impl AsRef<OsStr>`"),
];

fn check_owned_ref_param(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::Fn>()
        .filter(|function| {
            super::support::is_item_or_impl_fn(function) && !ctx.is_in_test(function)
        })
        .flat_map(|function| {
            function
                .param_list()
                .into_iter()
                .flat_map(|params| params.params())
                .filter_map(|param| {
                    let ty = param.ty()?;

                    owned_ref_target(ty.clone()).map(|(name, suggestion)| {
                        ctx.violation(
                            &ty,
                            format!("parameter typed `&{name}` — accept {suggestion} instead"),
                        )
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn owned_ref_target(ty: ast::Type) -> Option<(&'static str, &'static str)> {
    let ast::Type::RefType(reference) = ty else {
        return None;
    };

    if reference.mut_token().is_some() {
        return None;
    }

    let ast::Type::PathType(path) = reference.ty()? else {
        return None;
    };
    let last = path.path()?.segment()?.name_ref()?;

    OWNED_REF_SUGGESTIONS
        .iter()
        .copied()
        .find(|(name, _)| last.text() == *name)
}

crate::rulewright_ast_test!(check_owned_ref_param, {
    crate::example_tests!(EXAMPLES, check_owned_ref_param);
});
