#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::ast;

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "Future impl without Send assertion",
        code: "struct Foo;\nimpl Future for Foo { type Output = (); }",
        pass: false,
    },
    Example {
        label: "qualified Future impl without Send assertion",
        code: "struct Foo;\nimpl std::future::Future for Foo { type Output = (); }",
        pass: false,
    },
    Example {
        label: "Future impl with Send assertion",
        code: "struct Foo;\nimpl Future for Foo { type Output = (); }\nconst fn assert_send<T: Send>() {}\nconst _: () = assert_send::<Foo>();",
        pass: true,
    },
    Example {
        label: "two implementors, one unasserted",
        code: "struct Foo;\nstruct Bar;\nimpl Future for Foo { type Output = (); }\nimpl Future for Bar { type Output = (); }\nconst fn assert_send<T: Send>() {}\nconst _: () = assert_send::<Foo>();",
        pass: false,
    },
    Example {
        label: "no Future impl",
        code: "struct Foo;\nimpl Iterator for Foo { type Item = u8; }",
        pass: true,
    },
    Example {
        label: "Future impl in test module",
        code: "#[cfg(test)]\nmod tests {\n    struct Foo;\n    impl Future for Foo { type Output = (); }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    future_send_assert,
    "Require a compile-time `Send` assertion for every explicit `impl Future` in the same file.",
    "Explicitly declared futures silently turning !Send breaks Tokio and runtime-abstraction consumers; a const assertion catches the regression at compile time.",
    Low,
);

fn check_future_send_assert(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::Impl>()
        .filter(|item_impl| !ctx.is_in_test(item_impl))
        .filter_map(|item_impl| {
            let name = future_impl_target(&item_impl)?;

            (!is_asserted(ctx.file.contents, &name)).then(|| {
                ctx.violation(
                    &item_impl,
                    format!(
                        "`impl Future for {name}` without a compile-time Send assertion — add `const _: () = assert_send::<{name}>();`"
                    ),
                )
            })
        })
        .collect()
}

fn future_impl_target(item_impl: &ast::Impl) -> Option<String> {
    let ast::Type::PathType(trait_type) = item_impl.trait_()? else {
        return None;
    };

    if !super::support::path_last_is(trait_type.path()?, "Future") {
        return None;
    }

    super::support::type_name(item_impl.self_ty()?)
}

fn is_asserted(contents: &str, name: &str) -> bool {
    contents.match_indices("assert_send").any(|(idx, needle)| {
        let tail = contents.get(idx + needle.len()..).unwrap_or("");
        let window = tail.split('>').next().unwrap_or("");

        window
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .any(|word| word == name)
    })
}

crate::rulewright_ast_test!(check_future_send_assert, {
    crate::example_tests!(EXAMPLES, check_future_send_assert);

    #[gtest]
    fn two_unasserted_implementors_flag_twice() -> Result<()> {
        let v = run(
            "struct Foo;\nstruct Bar;\nimpl Future for Foo { type Output = (); }\nimpl Future for Bar { type Output = (); }",
        );
        verify_eq!(v.len(), 2)?;

        Ok(())
    }
});
