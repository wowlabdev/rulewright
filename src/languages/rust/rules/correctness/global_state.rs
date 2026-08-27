use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "static atomic",
        code: "static COUNTER: AtomicUsize = AtomicUsize::new(0);",
        pass: false,
    },
    Example {
        label: "static once lock",
        code: "static CACHE: OnceLock<String> = OnceLock::new();",
        pass: false,
    },
    Example {
        label: "nested interior mutability",
        code: "static SLOTS: [Option<Mutex<u8>>; 2] = [None, None];",
        pass: false,
    },
    Example {
        label: "thread local",
        code: "thread_local! {\n    static TLS: RefCell<u8> = RefCell::new(0);\n}",
        pass: false,
    },
    Example {
        label: "immutable str static",
        code: "static NAME: &str = \"project\";",
        pass: true,
    },
    Example {
        label: "immutable numeric static",
        code: "static LIMIT: usize = 64;",
        pass: true,
    },
    Example {
        label: "local mutex",
        code: "fn f() { let m = std::sync::Mutex::new(0); drop(m); }",
        pass: true,
    },
    Example {
        label: "static atomic in test module",
        code: "#[cfg(test)]\nmod tests {\n    static COUNTER: AtomicUsize = AtomicUsize::new(0);\n}",
        pass: true,
    },
];

crate::ast_rule!(
    global_state,
    "Flag `static` items with interior mutability and all `thread_local!` state.",
    "Mutable globals are secretly duplicated across linked crate versions and break test isolation; perf-only caches need a #rw suppression with reason.",
    Medium,
);

const INTERIOR_MUTABLE: &[&str] = &[
    "Cell", "LazyCell", "LazyLock", "Mutex", "OnceCell", "OnceLock", "RefCell", "RwLock",
];

fn interior_mutable_name(ty: &ast::Type) -> Option<String> {
    ty.syntax()
        .descendants()
        .filter_map(ast::NameRef::cast)
        .map(|name| name.text().to_string())
        .find(|name| name.starts_with("Atomic") || INTERIOR_MUTABLE.contains(&name.as_str()))
}

fn check_global_state(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let statics = ctx
        .nodes::<ast::Static>()
        .filter(|item| !ctx.is_in_test(item))
        .filter_map(|item| {
            let name = interior_mutable_name(&item.ty()?)?;
            let ident = item.name()?;

            Some(ctx.violation(
                &ident,
                format!(
                    "static `{ident}` contains interior mutability (`{name}`) — pass state explicitly instead of a global"
                ),
            ))
        });
    let macro_calls = ctx
        .nodes::<ast::MacroCall>()
        .filter(|call| !ctx.is_in_test(call));
    let thread_locals = macro_calls
        .filter(|call| {
            let name = call
                .path()
                .and_then(|path| path.segment())
                .and_then(|segment| segment.name_ref());

            name.is_some_and(|name| name.text() == "thread_local")
        })
        .map(|call| {
            ctx.violation(
                &call,
                "thread_local! creates hidden global state — pass state explicitly instead",
            )
        });

    statics.chain(thread_locals).collect()
}

crate::rulewright_ast_test!(check_global_state, {
    crate::example_tests!(EXAMPLES, check_global_state);
});
