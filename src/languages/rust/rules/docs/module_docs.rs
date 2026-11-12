#[cfg(test)]
use googletest::prelude::*;

use crate::{Example, FileCtx, Violation, violation};

/// Pass/fail cases checked against a library crate root.
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

crate::full_line_rule!(
    module_docs,
    "Require `//!` docs at the top of Rust library roots and module files.",
    "Crate and module docs are the entry points for navigating code: they say what a crate or module is for before a reader opens its items. Binary roots, build scripts, and test code are excluded.",
    Medium,
);

fn check_module_docs(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let Some(kind) = documented_file_kind(ctx) else {
        return Vec::new();
    };

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

    vec![violation(ctx.rel, 1, kind)]
}

fn documented_file_kind(ctx: &FileCtx<'_>) -> Option<&'static str> {
    let name = ctx.path.file_name().and_then(|value| value.to_str())?;
    let in_bin_directory = ctx
        .rel
        .rsplit('/')
        .nth(1)
        .is_some_and(|parent| parent == "bin");

    if name == "lib.rs" {
        Some("Rust library crate root must begin with `//!` crate docs")
    } else if ctx.test_only
        || ctx
            .path
            .extension()
            .is_none_or(|extension| extension != "rs")
        || name == "main.rs"
        || name == "build.rs"
        || in_bin_directory
    {
        None
    } else {
        Some("Rust module file must begin with `//!` module docs")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::check_source_at;

    const GATED_REL: &str = "crates/demo/src/lib.rs";

    #[gtest]
    fn examples() -> Result<()> {
        for ex in EXAMPLES {
            let violations = check_source_at(GATED_REL, ex.code, check_module_docs);

            verify_eq!(violations.is_empty(), ex.pass)?;
        }

        Ok(())
    }

    #[gtest]
    fn library_root_is_gated() -> Result<()> {
        let violations = check_source_at(GATED_REL, "pub mod x;", check_module_docs);

        verify_false!(violations.is_empty())?;

        Ok(())
    }

    #[gtest]
    fn module_files_are_gated() -> Result<()> {
        for rel in [
            "crates/demo/src/util.rs",
            "crates/demo/src/util/mod.rs",
            "crates/demo/src/bin/tool/args.rs",
        ] {
            let violations = check_source_at(rel, "pub fn f() {}", check_module_docs);

            verify_eq!(violations.len(), 1)?;
            verify_that!(violations[0].message, contains_substring("module docs"))?;
        }

        Ok(())
    }

    #[gtest]
    fn test_only_module_files_pass() -> Result<()> {
        let config = crate::Config::generate_default(&[]);
        let lines = ["pub fn f() {}"];
        let file = FileCtx {
            test_only: true,
            rel: "crates/demo/tests/support.rs",
            path: crate::Path::new("crates/demo/tests/support.rs"),
            package_name: None,
            package_publishable: None,
            lines: &lines,
            contents: "pub fn f() {}",
            config: &config,
        };

        verify_true!(check_module_docs(&file).is_empty())
    }

    #[gtest]
    fn binaries_build_scripts_and_non_rust_files_pass() -> Result<()> {
        let rels = [
            "crates/demo/src/main.rs",
            "crates/demo/src/bin/tool.rs",
            "crates/demo/build.rs",
            "crates/demo/src/template.txt",
        ];

        for rel in rels {
            let violations = check_source_at(rel, "pub fn f() {}", check_module_docs);

            verify_true!(violations.is_empty())?;
        }

        Ok(())
    }
}
