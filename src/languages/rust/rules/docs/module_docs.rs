#[cfg(test)]
use googletest::prelude::*;

use crate::{Example, FileCtx, Violation, violation};

/// Pass/fail cases checked against a gated `mod.rs` rel path.
#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "module docs on first line",
        code: "//! Module docs.\n\npub fn f() {}",
        pass: true,
    },
    Example {
        label: "no module docs",
        code: "pub fn f() {}",
        pass: false,
    },
    Example {
        label: "suppression directive before module docs",
        code: "// #rw(file: rust_panic) tooling fixture\n\n//! Module docs.\npub fn f() {}",
        pass: true,
    },
    Example {
        label: "inner attribute before module docs",
        code: "#![allow(dead_code)]\n//! Module docs.\npub fn f() {}",
        pass: true,
    },
    Example {
        label: "multi-line inner attribute before module docs",
        code: "#![allow(\n    dead_code\n)]\n//! Module docs.\npub fn f() {}",
        pass: true,
    },
    Example {
        label: "blank lines before module docs",
        code: "\n\n//! Module docs.\npub fn f() {}",
        pass: true,
    },
    Example {
        label: "regular comment before module docs",
        code: "// a stray comment\n//! Module docs.\npub fn f() {}",
        pass: false,
    },
    Example {
        label: "only items",
        code: "pub struct S;",
        pass: false,
    },
];

crate::line_rule!(
    module_docs,
    "Require `//!` module docs at the top of `lib.rs` and `mod.rs` files.",
    "Module docs are the entry point for API navigation; each module file should say what it contains.",
    Medium,
);

fn check_module_docs(ctx: &FileCtx<'_>) -> Vec<Violation> {
    if ctx.package_name.is_none() || !is_module_entry(ctx) {
        return Vec::new();
    }

    let mut in_attr = false;

    for line in ctx.lines {
        let trimmed = line.trim();

        if in_attr {
            in_attr = !trimmed.ends_with(']');
            continue;
        }

        if trimmed.starts_with("//!") {
            return Vec::new();
        }

        if trimmed.is_empty() || trimmed.starts_with("// #rw(") {
            continue;
        }

        if trimmed.starts_with("#![") {
            in_attr = !trimmed.ends_with(']');
            continue;
        }

        break;
    }

    vec![violation(
        ctx.rel,
        1,
        "`lib.rs`/`mod.rs` must begin with `//!` module docs",
    )]
}

fn is_module_entry(ctx: &FileCtx<'_>) -> bool {
    let Some(name) = ctx.path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    name == "lib.rs" || name == "mod.rs"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::check_source_at;

    const GATED_REL: &str = "crates/demo/src/util/mod.rs";

    #[gtest]
    fn examples() -> Result<()> {
        for ex in EXAMPLES {
            let violations = check_source_at(GATED_REL, ex.code, check_module_docs);

            verify_eq!(violations.is_empty(), ex.pass)?;
        }

        Ok(())
    }

    #[gtest]
    fn lib_rs_is_gated() -> Result<()> {
        let violations = check_source_at("crates/demo/src/lib.rs", "pub mod x;", check_module_docs);

        verify_false!(violations.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn ungated_paths_pass() -> Result<()> {
        let rels = ["test.rs", "crates/demo/src/util.rs", "crates/demo/mod.rs"];

        for rel in rels {
            let violations = check_source_at(rel, "pub fn f() {}", check_module_docs);

            verify_true!(violations.is_empty())?;
        }

        Ok(())
    }
}
