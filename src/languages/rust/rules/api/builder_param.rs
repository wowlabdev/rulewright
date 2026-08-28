use ra_ap_syntax::ast;

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "builder parameter",
        code: "fn make(b: WidgetBuilder) {}",
        pass: false,
    },
    Example {
        label: "factory reference parameter",
        code: "fn make(f: &WidgetFactory) {}",
        pass: false,
    },
    Example {
        label: "builder parameter on method",
        code: "struct App;\nimpl App {\n    fn install(&self, b: WidgetBuilder) {}\n}",
        pass: false,
    },
    Example {
        label: "mutable builder accumulation context",
        code: "fn add_fields(builder: &mut WidgetBuilder) { builder.field(1); }",
        pass: true,
    },
    Example {
        label: "closure factory parameter",
        code: "fn make(f: impl Fn() -> Widget) {}",
        pass: true,
    },
    Example {
        label: "plain parameter",
        code: "fn make(w: Widget) {}",
        pass: true,
    },
    Example {
        label: "builder parameter in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn make(b: WidgetBuilder) {}\n}",
        pass: true,
    },
];

crate::ast_rule!(
    builder_param,
    "Flag owned/shared parameters typed `*Builder`/`*Factory` — ask for `impl Fn() -> T` instead.",
    "Accepting an owned builder or shared factory as a parameter often imports OO indirection; an impl Fn() -> T expresses repeatable instantiation idiomatically. A mutable builder reference is an explicit accumulation context and should be passed directly.",
    Low,
);

fn check_builder_param(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::Fn>()
        .filter(|function| !ctx.is_in_test(function))
        .flat_map(|function| {
            function
                .param_list()
                .into_iter()
                .flat_map(|params| params.params())
                .filter_map(|param| {
                    let ty = param.ty()?;
                    let ident = weasel_param_ident(&ty)?;

                    Some(ctx.violation(
                        &ty,
                        format!(
                            "parameter of type `{ident}` — accept `impl Fn() -> T` instead of a builder/factory"
                        ),
                    ))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn weasel_param_ident(ty: &ast::Type) -> Option<String> {
    let inner = match ty {
        ast::Type::RefType(reference) if reference.mut_token().is_some() => return None,
        ast::Type::RefType(reference) => reference.ty()?,
        _ => ty.clone(),
    };
    let ast::Type::PathType(path) = inner else {
        return None;
    };
    let name = path.path()?.segment()?.name_ref()?.text().to_string();

    (name.ends_with("Builder") || name.ends_with("Factory")).then_some(name)
}

crate::rulewright_ast_test!(check_builder_param, {
    crate::example_tests!(EXAMPLES, check_builder_param);
});
