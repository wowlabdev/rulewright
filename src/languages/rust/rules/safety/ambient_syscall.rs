use ra_ap_syntax::ast;

use super::super::support::Imports;
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
    let imports = Imports::of(ctx);

    ctx.nodes::<ast::CallExpr>()
        .filter(|call| !ctx.is_in_test(call))
        .filter_map(|call| {
            let ast::Expr::PathExpr(path_expr) = call.expr()? else {
                return None;
            };
            let path = path_expr.path()?;
            let segments = imports.resolve(path_segments(&path));

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
