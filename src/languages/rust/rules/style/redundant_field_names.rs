use ra_ap_syntax::{AstNode, ast};

use crate::{AstCtx, Example, Fix, Violation, infra::parse};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "redundant field name",
        code: "struct S { x: i32 }\nfn f(x: i32) -> S { S { x: x } }",
        pass: false,
    },
    Example {
        label: "shorthand field init",
        code: "struct S { x: i32 }\nfn f(x: i32) -> S { S { x } }",
        pass: true,
    },
    Example {
        label: "different name and value",
        code: "struct S { x: i32 }\nfn f(y: i32) -> S { S { x: y } }",
        pass: true,
    },
    Example {
        label: "redundant in test",
        code: "#[cfg(test)]\nmod tests {\n    struct S { x: i32 }\n    fn t(x: i32) -> S { S { x: x } }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    redundant_field_names,
    "Flag `Foo { x: x }` — use shorthand `Foo { x }` instead.",
    "Rust supports field init shorthand (Foo { x } instead of Foo { x: x }). The long form is needless noise.",
    Low,
    fix_redundant_field_names,
);

fn check_redundant_field_names(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::RecordExprField>()
        .filter(|field| !ctx.is_in_test(field) && field.colon_token().is_some())
        .filter_map(|field| {
            let name = field.name_ref()?;
            let ast::Expr::PathExpr(path) = field.expr()? else {
                return None;
            };

            (path.syntax().text().to_string() == name.text()).then(|| {
                let name_text = name.text();

                ctx.violation(
                    &name,
                    format!(
                        "redundant field initializer `{name_text}: {name_text}` — use shorthand `{name_text}`"
                    ),
                )
            })
        })
        .collect()
}

fn fix_redundant_field_names(ctx: &AstCtx<'_>, v: &Violation) -> Option<Fix> {
    let line = ctx.file.line(v.line)?;
    let name = parse::redundant_field_name(&v.message)?;
    let redundant = format!("{name}: {name}");

    line.contains(&redundant)
        .then(|| Fix::replace_line(v.line, line.replacen(&redundant, name, 1)))
}

crate::rulewright_ast_test!(check_redundant_field_names, {
    crate::example_tests!(EXAMPLES, check_redundant_field_names);
    crate::fix_tests!(ast, check_redundant_field_names, fix_redundant_field_names);
});
