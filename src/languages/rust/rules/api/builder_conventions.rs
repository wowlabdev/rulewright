use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName, HasVisibility, VisibilityKind},
};
use std::collections::{HashMap, HashSet};

use super::support::type_name;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "conventional builder",
        code: "pub struct Foo;\npub struct FooBuilder { size: u32 }\nimpl Foo {\n    pub fn builder() -> FooBuilder {\n        FooBuilder { size: 0 }\n    }\n}\nimpl FooBuilder {\n    pub fn size(mut self, size: u32) -> Self {\n        self.size = size;\n        self\n    }\n    pub fn build(self) -> Foo {\n        Foo\n    }\n}",
        pass: true,
    },
    Example {
        label: "builder without build method",
        code: "pub struct ConnBuilder { retries: u32 }\nimpl ConnBuilder {\n    pub fn retries(mut self, retries: u32) -> Self {\n        self.retries = retries;\n        self\n    }\n}",
        pass: false,
    },
    Example {
        label: "public new on builder",
        code: "pub struct Conn;\npub struct ConnBuilder;\nimpl ConnBuilder {\n    pub fn new() -> Self {\n        ConnBuilder\n    }\n    pub fn build(self) -> Conn {\n        Conn\n    }\n}",
        pass: false,
    },
    Example {
        label: "builder is the public entry point and converts into a draft",
        code: "pub struct PlanBuilder; pub struct Draft; impl PlanBuilder { pub fn new() -> Self { Self } pub fn into_draft(self) -> Result<Draft, Error> { Ok(Draft) } }",
        pass: true,
    },
    Example {
        label: "set_ prefixed setter",
        code: "pub struct ConnBuilder { port: u16 }\nimpl ConnBuilder {\n    pub fn set_port(mut self, port: u16) -> Self {\n        self.port = port;\n        self\n    }\n    pub fn build(self) -> u16 {\n        self.port\n    }\n}",
        pass: false,
    },
    Example {
        label: "with_ prefixed setter",
        code: "pub struct ConnBuilder { port: u16 }\nimpl ConnBuilder {\n    pub fn with_port(mut self, port: u16) -> Self {\n        self.port = port;\n        self\n    }\n    pub fn build(self) -> u16 {\n        self.port\n    }\n}",
        pass: false,
    },
    Example {
        label: "borrowing setter",
        code: "pub struct ConnBuilder { port: u16 }\nimpl ConnBuilder {\n    pub fn port(&mut self, port: u16) -> &mut Self {\n        self.port = port;\n        self\n    }\n    pub fn build(self) -> u16 {\n        self.port\n    }\n}",
        pass: false,
    },
    Example {
        label: "imperative builder mutation",
        code: "pub struct ConnBuilder { port: u16 }\nimpl ConnBuilder {\n    pub fn set_port(&mut self, port: u16) { self.port = port; }\n    pub fn build(self) -> u16 { self.port }\n}",
        pass: true,
    },
    Example {
        label: "buildable type without builder shortcut",
        code: "pub struct Conn;\npub struct ConnBuilder;\nimpl ConnBuilder {\n    pub fn build(self) -> Conn {\n        Conn\n    }\n}",
        pass: false,
    },
    Example {
        label: "builder without same-file impl",
        code: "pub struct ConnBuilder;",
        pass: true,
    },
    Example {
        label: "private builder",
        code: "struct ConnBuilder;\nimpl ConnBuilder {\n    fn new() -> Self {\n        ConnBuilder\n    }\n}",
        pass: true,
    },
    Example {
        label: "builder in test module",
        code: "#[cfg(test)]\nmod tests {\n    pub struct ConnBuilder;\n    impl ConnBuilder {\n        pub fn new() -> Self {\n            ConnBuilder\n        }\n    }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    builder_conventions,
    "Enforce builder conventions: chainable by-value setters named `x()`, a terminal build/finish/into_* method, and `X::builder()` when a separate `X` product type exists.",
    "Consistent fluent setters and an explicit terminal transition make construction easy to follow. A standalone builder may expose new(); the X::builder() shortcut is expected only when X is a same-module product type.",
    Medium,
);

fn check_builder_conventions(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let (pub_structs, methods) = collect_items(ctx);

    ctx.nodes::<ast::Struct>()
        .filter(|item| !ctx.is_in_test(item) && is_pub(item.visibility()))
        .flat_map(|item| check_builder(ctx, &item, &pub_structs, &methods))
        .collect()
}

fn collect_items(ctx: &AstCtx<'_>) -> (HashSet<String>, HashMap<String, Vec<ast::Fn>>) {
    let mut structs = HashSet::default();
    let mut methods: HashMap<String, Vec<ast::Fn>> = HashMap::default();

    for item in ctx.root.syntax().children().filter_map(ast::Item::cast) {
        match item {
            ast::Item::Struct(item) if is_pub(item.visibility()) => {
                if let Some(name) = item.name() {
                    structs.insert(name.text().to_string());
                }
            }

            ast::Item::Impl(item) if item.trait_().is_none() => {
                let Some(name) = item.self_ty().and_then(|ty| type_name(&ty)) else {
                    continue;
                };

                methods.entry(name).or_default().extend(
                    item.assoc_item_list()
                        .into_iter()
                        .flat_map(|list| list.assoc_items())
                        .filter_map(|assoc| match assoc {
                            ast::AssocItem::Fn(function) => Some(function),
                            _ => None,
                        }),
                );
            }

            _ => {}
        }
    }

    (structs, methods)
}

fn is_chainable_setter(function: &ast::Fn, builder: &str) -> bool {
    has_receiver(function)
        && returns_self(function, builder)
        && function.name().is_some_and(|name| name.text() != "build")
}

fn has_receiver(function: &ast::Fn) -> bool {
    function
        .param_list()
        .is_some_and(|params| params.self_param().is_some())
}

fn takes_self_by_value(function: &ast::Fn) -> bool {
    function
        .param_list()
        .and_then(|params| params.self_param())
        .is_some_and(|receiver| receiver.amp_token().is_none())
}

fn returns_self(function: &ast::Fn, builder: &str) -> bool {
    let Some(mut ty) = function.ret_type().and_then(|ret| ret.ty()) else {
        return false;
    };

    if let ast::Type::RefType(reference) = ty {
        let Some(inner) = reference.ty() else {
            return false;
        };

        ty = inner;
    }

    type_name(&ty).is_some_and(|name| name == "Self" || name == builder)
}

fn is_builder_ctor_name(function: &ast::Fn) -> bool {
    let Some(name) = function.name().map(|name| name.text().to_string()) else {
        return false;
    };

    name == "builder" || name.starts_with("builder_")
}

fn is_terminal_name(function: &ast::Fn) -> bool {
    function.name().is_some_and(|name| {
        matches!(name.text().as_str(), "build" | "try_build" | "finish")
            || name.text().starts_with("into_")
    })
}

fn check_builder(
    ctx: &AstCtx<'_>,
    item: &ast::Struct,
    pub_structs: &HashSet<String>,
    methods: &HashMap<String, Vec<ast::Fn>>,
) -> Vec<Violation> {
    let Some(name_node) = item.name() else {
        return Vec::new();
    };
    let name = name_node.text().to_string();
    let Some(base) = name.strip_suffix("Builder") else {
        return Vec::new();
    };
    let Some(own) = methods.get(&name).filter(|own| !own.is_empty()) else {
        return Vec::new();
    };
    let mut out = Vec::new();

    if !own.iter().any(is_terminal_name) {
        out.push(ctx.violation(
            &name_node,
            format!("builder `{name}` has no terminal `build()`/`finish()`/`into_*()` method"),
        ));
    }

    for function in own {
        let Some(fn_node) = function.name() else {
            continue;
        };
        let fn_name = fn_node.text();

        if fn_name == "new" && is_pub(function.visibility()) && pub_structs.contains(base) {
            out.push(ctx.violation(
                &fn_node,
                format!("`{name}::new` should not be public — provide `{base}::builder()` instead"),
            ));
        }

        if (fn_name.starts_with("set_") || fn_name.starts_with("with_"))
            && is_chainable_setter(function, &name)
        {
            out.push(ctx.violation(&fn_node, format!("builder setter `{name}::{fn_name}` — setters are bare `x()`, not `set_x()`/`with_x()`")));
        }

        if is_chainable_setter(function, &name) && !takes_self_by_value(function) {
            out.push(ctx.violation(
                &fn_node,
                format!("builder setter `{name}::{fn_name}` must take `self` by value to chain"),
            ));
        }
    }

    if !base.is_empty()
        && pub_structs.contains(base)
        && !methods
            .get(base)
            .is_some_and(|methods| methods.iter().any(is_builder_ctor_name))
    {
        out.push(ctx.violation(
            &name_node,
            format!("`{base}` should provide a `builder()` shortcut returning `{name}`"),
        ));
    }

    out
}

fn is_pub(visibility: Option<ast::Visibility>) -> bool {
    visibility.is_some_and(|vis| matches!(vis.kind(), VisibilityKind::Pub))
}

crate::rulewright_ast_test!(check_builder_conventions, {
    crate::example_tests!(EXAMPLES, check_builder_conventions);
});
