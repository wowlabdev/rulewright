use std::collections::BTreeMap;

use ra_ap_syntax::ast::{self, HasName};

use super::support::path_segments;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "ambient fs read",
        code: "fn load() { let _ = std::fs::read(\"cfg.toml\"); }",
        pass: false,
    },
    Example {
        label: "ambient fs write via import",
        code: "use std::fs;\nfn save() { let _ = fs::write(\"out.bin\", b\"data\"); }",
        pass: false,
    },
    Example {
        label: "ambient File open",
        code: "fn open() { let _ = std::fs::File::open(\"cfg.toml\"); }",
        pass: false,
    },
    Example {
        label: "ambient clock",
        code: "fn stamp() { let _ = std::time::Instant::now(); }",
        pass: false,
    },
    Example {
        label: "ambient system time",
        code: "use std::time::SystemTime;\nfn stamp() { let _ = SystemTime::now(); }",
        pass: false,
    },
    Example {
        label: "ambient env read",
        code: "fn cfg() { let _ = std::env::var(\"MODE\"); }",
        pass: false,
    },
    Example {
        label: "ambient network connect",
        code: "fn dial(addr: &str) { let _ = std::net::TcpStream::connect(addr); }",
        pass: false,
    },
    Example {
        label: "ambient entropy",
        code: "fn roll() -> u32 { rand::random() }",
        pass: false,
    },
    Example {
        label: "ambient thread_rng import",
        code: "use rand::thread_rng;\nfn roll() { let _ = thread_rng(); }",
        pass: false,
    },
    Example {
        label: "aliased clock import",
        code: "use std::time::Instant as Clock;\nfn stamp() { let _ = Clock::now(); }",
        pass: false,
    },
    Example {
        label: "aliased fs function",
        code: "use std::fs::read as load;\nfn cfg() { let _ = load(\"cfg.toml\"); }",
        pass: false,
    },
    Example {
        label: "thread spawn via module import",
        code: "use std::thread;\nfn run() { let _ = thread::spawn(|| {}); }",
        pass: false,
    },
    Example {
        label: "thread sleep",
        code: "fn wait() { std::thread::sleep(std::time::Duration::from_millis(1)); }",
        pass: false,
    },
    Example {
        label: "process spawn",
        code: "use std::process::Command;\nfn run() { let _ = Command::new(\"ls\"); }",
        pass: false,
    },
    Example {
        label: "stdin and args",
        code: "fn input() { let _ = std::io::stdin(); let _ = std::env::args(); }",
        pass: false,
    },
    Example {
        label: "foreign fs module import",
        code: "use custom::fs;\nfn load() { let _ = fs::read(1); }",
        pass: true,
    },
    Example {
        label: "foreign Command type",
        code: "use clap::Command;\nfn cli() { let _ = Command::new(\"tool\"); }",
        pass: true,
    },
    Example {
        label: "injected clock is fine",
        code: "fn stamp(clock: &dyn Clock) { let _ = clock.now(); }",
        pass: true,
    },
    Example {
        label: "unrelated fs module",
        code: "fn load() { let _ = custom::fs::read(1); }",
        pass: true,
    },
    Example {
        label: "syscall in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn t() { let _ = std::time::Instant::now(); }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    ambient_syscall,
    "Flag ambient I/O, clock, env, thread, process, and entropy calls outside test code.",
    "Syscalls called ambiently cannot be mocked, making edge cases untestable — inject them through an abstraction. Calls are matched by path, following `use` imports and aliases in the same file; method calls, re-exports, and glob imports are not resolved. The rule applies to libraries and binaries alike: scope it to library code with `only` or `ignore` (for example `ignore = [\"**/src/main.rs\", \"**/src/bin/**\"]`).",
    Medium,
);

fn check_ambient_syscall(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let imports = imported_paths(ctx);

    ctx.nodes::<ast::CallExpr>()
        .filter(|call| !ctx.is_in_test(call))
        .filter_map(|call| {
            let ast::Expr::PathExpr(path_expr) = call.expr()? else {
                return None;
            };
            let path = path_expr.path()?;
            let segments = resolve(&imports, path_segments(&path));

            is_ambient_call(&segments).then(|| {
                ctx.violation(
                    &path_expr,
                    format!(
                        "ambient syscall `{}` — inject I/O, clocks, and entropy through \
                         a mockable abstraction",
                        segments.join("::")
                    ),
                )
            })
        })
        .collect()
}

/// Map each name a `use` item binds to the path it imports; ambiguous names are dropped.
fn imported_paths(ctx: &AstCtx<'_>) -> BTreeMap<String, Vec<String>> {
    let mut imports = BTreeMap::<String, Option<Vec<String>>>::new();

    let mut pending = ctx
        .nodes::<ast::Use>()
        .filter_map(|item| item.use_tree())
        .map(|tree| (tree, Vec::new()))
        .collect::<Vec<_>>();

    while let Some((tree, prefix)) = pending.pop() {
        let mut path = prefix;

        if let Some(own) = tree.path() {
            path.extend(path_segments(&own));
        }

        if let Some(list) = tree.use_tree_list() {
            pending.extend(list.use_trees().map(|child| (child, path.clone())));
        } else if tree.star_token().is_none() {
            bind(&tree, path, &mut imports);
        }
    }

    imports
        .into_iter()
        .filter_map(|(name, path)| Some((name, path?)))
        .collect()
}

fn bind(
    tree: &ast::UseTree,
    mut path: Vec<String>,
    imports: &mut BTreeMap<String, Option<Vec<String>>>,
) {
    if path.last().is_some_and(|last| last == "self") {
        path.pop();
    }

    let name = match tree.rename() {
        Some(rename) => match rename.name() {
            Some(name) => name.text().to_string(),
            None => return,
        },

        None => match path.last() {
            Some(last) => last.clone(),
            None => return,
        },
    };

    if path.len() < 2 || name == "_" {
        return;
    }

    let entry = imports.entry(name).or_insert_with(|| Some(path.clone()));

    if entry.as_ref().is_some_and(|known| *known != path) {
        *entry = None;
    }
}

fn resolve(imports: &BTreeMap<String, Vec<String>>, segments: Vec<String>) -> Vec<String> {
    let Some((first, rest)) = segments.split_first() else {
        return segments;
    };

    match imports.get(first) {
        Some(path) => path.iter().chain(rest).cloned().collect(),
        None => segments,
    }
}

fn is_ambient_call(segments: &[String]) -> bool {
    match segments {
        [] => false,

        [only] => only == "thread_rng",

        [.., a, b] => {
            is_type_syscall(a, b)
                || is_std_module_call(segments, a, b)
                || is_std_process_spawn(segments)
                || (a == "rand" && (b == "thread_rng" || b == "random"))
        }
    }
}

fn is_std_process_spawn(segments: &[String]) -> bool {
    let tail = ["process", "Command", "new"];

    segments.ends_with(&tail.map(String::from))
        && matches!(segments.len(), 3 | 4)
        && (segments.len() == 3 || segments.first().is_some_and(|root| root == "std"))
}

fn is_type_syscall(ty: &str, method: &str) -> bool {
    matches!(
        (ty, method),
        ("File", "open" | "create")
            | ("TcpStream", "connect")
            | ("TcpListener" | "UdpSocket", "bind")
            | ("SystemTime" | "Instant", "now")
    )
}

/// An unprefixed `module::fn` call is exactly two path segments.
const MODULE_CALL_SEGMENTS: usize = 2;

fn is_std_module_call(segments: &[String], module: &str, name: &str) -> bool {
    let rooted = segments.len() == MODULE_CALL_SEGMENTS
        || segments.first().is_some_and(|root| root == "std");

    if !rooted {
        return false;
    }

    match module {
        "fs" => true,

        "env" => matches!(
            name,
            "var" | "var_os" | "vars" | "vars_os" | "args" | "args_os" | "current_dir"
        ),

        "thread" => matches!(name, "spawn" | "sleep" | "scope"),

        "io" => name == "stdin",

        _ => false,
    }
}

crate::rulewright_ast_test!(check_ambient_syscall, {
    crate::example_tests!(EXAMPLES, check_ambient_syscall);
});
