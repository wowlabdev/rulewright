use ra_ap_syntax::ast;

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "File parameter by value",
        code: "fn parse(file: File) {}",
        pass: false,
    },
    Example {
        label: "File parameter by mutable reference",
        code: "fn parse(file: &mut File) {}",
        pass: false,
    },
    Example {
        label: "TcpStream parameter",
        code: "fn read_frame(stream: TcpStream) {}",
        pass: false,
    },
    Example {
        label: "fully qualified TcpStream parameter",
        code: "fn read_frame(stream: std::net::TcpStream) {}",
        pass: false,
    },
    Example {
        label: "Stdout parameter",
        code: "fn log_to(out: Stdout) {}",
        pass: false,
    },
    Example {
        label: "File in impl method",
        code: "struct S;\nimpl S {\n    fn parse(&self, file: &File) {}\n}",
        pass: false,
    },
    Example {
        label: "impl Read parameter",
        code: "fn parse(data: impl std::io::Read) {}",
        pass: true,
    },
    Example {
        label: "byte slice parameter",
        code: "fn parse(data: &[u8]) {}",
        pass: true,
    },
    Example {
        label: "File as return type is fine",
        code: "fn open(path: &std::path::Path) -> File { make(path) }",
        pass: true,
    },
    Example {
        label: "unlisted type passes",
        code: "fn parse(reader: BufReader<u8>) {}",
        pass: true,
    },
    Example {
        label: "File parameter in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn parse(file: File) {}\n}",
        pass: true,
    },
    Example {
        label: "syntax tree parameter",
        code: "fn inspect(file: &syn::File) {}",
        pass: true,
    },
];

crate::ast_rule!(
    concrete_io_param,
    "Flag fn parameters typed as concrete I/O handles like `File` or `TcpStream`.",
    "Concrete I/O parameter types couple logic to one byte source; `impl std::io::Read`/`impl std::io::Write` works with files, sockets, and buffers alike.",
    Low,
    params {
        types: [String] = ["File", "Stdin", "Stdout", "TcpStream", "UnixStream"]
    },
);

fn check_concrete_io_param(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let types = ctx
        .file
        .config
        .get_str_array("rust_concrete_io_param", &PARAMS[0]);

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

                    io_type_name(ty.clone(), &types).map(|name| {
                        ctx.violation(
                            &ty,
                            format!(
                                "parameter typed `{name}` couples the function to concrete I/O — accept `impl std::io::Read`/`impl std::io::Write` (sans-io) instead"
                            ),
                        )
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn io_type_name(ty: ast::Type, types: &[String]) -> Option<String> {
    let path = match ty {
        ast::Type::RefType(reference) => match reference.ty()? {
            ast::Type::PathType(path) => path.path()?,
            _ => return None,
        },
        ast::Type::PathType(path) => path.path()?,
        _ => return None,
    };
    let segments = super::support::path_names(path);
    let last = segments.last()?;

    if segments.len() > 1 && !is_std_io_path(&segments) {
        return None;
    }

    types.iter().find(|name| *name == last).cloned()
}

fn is_std_io_path(segments: &[String]) -> bool {
    const GRANDPARENT_SEGMENT: usize = 2;

    let type_name = segments.last().map(String::as_str);

    match type_name {
        Some("File") => segments
            .iter()
            .rev()
            .nth(1)
            .is_some_and(|module| module == "fs"),

        Some("Stdin" | "Stdout") => segments
            .iter()
            .rev()
            .nth(1)
            .is_some_and(|module| module == "io"),

        Some("TcpStream") => segments
            .iter()
            .rev()
            .nth(1)
            .is_some_and(|module| module == "net"),

        Some("UnixStream") => {
            segments
                .iter()
                .rev()
                .nth(1)
                .is_some_and(|module| module == "net")
                && segments
                    .iter()
                    .rev()
                    .nth(GRANDPARENT_SEGMENT)
                    .is_some_and(|module| module == "unix")
        }
        _ => false,
    }
}

crate::rulewright_ast_test!(check_concrete_io_param, {
    crate::example_tests!(EXAMPLES, check_concrete_io_param);
});
