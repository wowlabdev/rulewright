use ra_ap_syntax::ast;

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "println in library",
        code: "fn f() { println!(\"hello\"); }",
        pass: false,
    },
    Example {
        label: "eprintln in library",
        code: "fn f() { eprintln!(\"error\"); }",
        pass: false,
    },
    Example {
        label: "print in library",
        code: "fn f() { print!(\"hello\"); }",
        pass: false,
    },
    Example {
        label: "eprint in library",
        code: "fn f() { eprint!(\"error\"); }",
        pass: false,
    },
    Example {
        label: "println in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn t() { println!(\"debug\"); }\n}",
        pass: true,
    },
    Example {
        label: "no println",
        code: "fn f() { let x = 1; }",
        pass: true,
    },
    Example {
        label: "println in string literal",
        code: r#"fn f() { let s = "println!(value)"; }"#,
        pass: true,
    },
];

crate::ast_rule!(
    println,
    "Ban `println!`/`eprintln!`/`print!`/`eprint!` outside test code.",
    "Console printing bypasses structured logging in libraries and services. Use tracing or an explicit output abstraction there; scope CLI output, examples, and benchmarks out of this policy instead of deleting observable behavior.",
    Medium,
);

const BANNED_MACROS: &[&str] = &["println", "eprintln", "print", "eprint"];

fn check_println(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::MacroCall>()
        .filter(|call| !ctx.is_in_test(call))
        .filter_map(|call| {
            let name = unqualified_macro_name(&call)?;

            BANNED_MACROS.contains(&name.as_str()).then(|| {
                ctx.violation(
                    &call,
                    format!(
                        "{name}!() outside test code (use tracing, an explicit output abstraction, or returned errors)"
                    ),
                )
            })
        })
        .collect()
}

fn unqualified_macro_name(call: &ast::MacroCall) -> Option<String> {
    let path = call.path()?;

    if path.qualifier().is_some() {
        return None;
    }

    path.segment()?
        .name_ref()
        .map(|name| name.text().to_string())
}

crate::rulewright_ast_test!(check_println, {
    crate::example_tests!(EXAMPLES, check_println);

    #[test]
    fn rule_is_diagnostic_only() {
        let rule = inventory::iter::<crate::Rule>
            .into_iter()
            .find(|rule| rule.info.name == "rust_println")
            .expect("rust_println is registered");

        assert!(rule.fix.is_none());
    }
});
