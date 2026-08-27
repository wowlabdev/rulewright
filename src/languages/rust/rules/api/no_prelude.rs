#[cfg(test)]
use googletest::prelude::*;

use crate::{Example, FileCtx, Violation, infra::scanner, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "pub prelude declaration",
        code: "pub mod prelude;",
        pass: false,
    },
    Example {
        label: "private prelude declaration",
        code: "mod prelude;",
        pass: false,
    },
    Example {
        label: "inline prelude module",
        code: "pub mod prelude {\n    pub struct Token;\n}",
        pass: false,
    },
    Example {
        label: "similarly named module",
        code: "mod prelude_ext;",
        pass: true,
    },
    Example {
        label: "commented declaration",
        code: "// mod prelude;",
        pass: true,
    },
    Example {
        label: "string mention",
        code: "fn f() { let s = \"mod prelude;\"; }",
        pass: true,
    },
    Example {
        label: "regular module",
        code: "pub mod parsing;",
        pass: true,
    },
];

crate::line_rule!(
    no_prelude,
    "Ban `prelude` module declarations and `prelude.rs`/`prelude/mod.rs` files.",
    "Preludes invite glob imports that collide across crates and paper over bad module design.",
    High,
);

fn check_no_prelude(ctx: &FileCtx<'_>) -> Vec<Violation> {
    if ctx.package_name.is_none() {
        return Vec::new();
    }

    let mut out = Vec::new();

    if ctx.rel.ends_with("/prelude.rs") || ctx.rel.ends_with("/prelude/mod.rs") {
        out.push(violation(
            ctx.rel,
            1,
            "prelude module file — crates must not define preludes",
        ));
    }

    for (index, line) in ctx.lines.iter().enumerate() {
        if declares_prelude(line) {
            out.push(violation(
                ctx.rel,
                index + 1,
                "`prelude` module declaration — crates must not define preludes",
            ));
        }
    }

    out
}

fn declares_prelude(line: &str) -> bool {
    let code = scanner::code_only(line);
    let trimmed = code.trim_start();
    let rest = trimmed
        .strip_prefix("pub mod prelude")
        .or_else(|| trimmed.strip_prefix("mod prelude"));

    match rest {
        Some(rest) => matches!(rest.trim_start().chars().next(), None | Some(';' | '{')),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gtest]
    fn examples() -> Result<()> {
        for ex in EXAMPLES {
            let violations = crate::test_support::check_source_at(
                "crates/demo/src/lib.rs",
                ex.code,
                check_no_prelude,
            );

            verify_eq!(violations.is_empty(), ex.pass)?;
        }

        Ok(())
    }

    #[gtest]
    fn prelude_file_paths_flagged_on_line_one() -> Result<()> {
        for rel in [
            "crates/demo/src/prelude.rs",
            "crates/demo/src/prelude/mod.rs",
        ] {
            let v =
                crate::test_support::check_source_at(rel, "pub struct Token;", check_no_prelude);

            verify_eq!(v.len(), 1)?;
            verify_eq!(v[0].line, 1)?;
        }

        Ok(())
    }

    #[gtest]
    fn regular_file_path_passes() -> Result<()> {
        let v = crate::test_support::check_source_at(
            "crates/demo/src/preludes.rs",
            "pub struct Token;",
            check_no_prelude,
        );

        verify_true!(v.is_empty())?;

        Ok(())
    }
}
