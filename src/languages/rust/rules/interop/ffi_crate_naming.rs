#[cfg(test)]
use googletest::prelude::*;

use crate::{Example, FileCtx, Violation, infra::scanner, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "no_mangle export in non-ffi crate",
        code: "#[no_mangle]\npub extern \"C\" fn service_tick() {}",
        pass: false,
    },
    Example {
        label: "unsafe no_mangle export in non-ffi crate",
        code: "#[unsafe(no_mangle)]\npub extern \"C\" fn service_tick() {}",
        pass: false,
    },
    Example {
        label: "foreign block in non-sys crate",
        code: "extern \"C\" {\n    fn native_call();\n}",
        pass: false,
    },
    Example {
        label: "plain rust fn",
        code: "pub fn tick() {}",
        pass: true,
    },
    Example {
        label: "no_mangle without extern C fn",
        code: "#[no_mangle]\npub static VERSION: u32 = 1;",
        pass: true,
    },
    Example {
        label: "extern C fn without no_mangle",
        code: "pub extern \"C\" fn callback() {}",
        pass: true,
    },
    Example {
        label: "wasm_bindgen crate uses its own convention",
        code: "use wasm_bindgen::prelude::JsValue;\n#[no_mangle]\npub extern \"C\" fn service_tick() {}",
        pass: true,
    },
];

crate::line_rule!(
    ffi_crate_naming,
    "Require `-ffi` naming for crates exporting C symbols and `-sys` naming for crates linking foreign C items.",
    "The `-ffi` (export) and `-sys` (import) suffixes make a crate's FFI role immediately recognizable across projects.",
    Low,
);

#[derive(Default)]
struct FfiUsage {
    no_mangle: bool,
    extern_fn_line: Option<usize>,
    foreign_block_line: Option<usize>,
}

fn scan_ffi_usage(lines: &[&str]) -> FfiUsage {
    let mut usage = FfiUsage::default();

    for (index, line) in lines.iter().enumerate() {
        let code = scanner::code_only(line);

        if code.contains("#[no_mangle]") || code.contains("#[unsafe(no_mangle)]") {
            usage.no_mangle = true;
        }

        if !code.contains("extern") {
            continue;
        }

        if usage.extern_fn_line.is_none() && line.contains("extern \"C\" fn") {
            usage.extern_fn_line = Some(index + 1);
        }

        if usage.foreign_block_line.is_none() && line.contains("extern \"C\" {") {
            usage.foreign_block_line = Some(index + 1);
        }
    }

    usage
}

fn check_ffi_crate_naming(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let Some(name) = ctx.package_name else {
        return Vec::new();
    };

    if super::wasm_exempt(ctx.contents) {
        return Vec::new();
    }

    let usage = scan_ffi_usage(ctx.lines);
    let mut out = Vec::new();

    if usage.no_mangle
        && !super::is_ffi_crate(name)
        && let Some(line) = usage.extern_fn_line
    {
        out.push(violation(
                ctx.rel,
                line,
                format!("crate `{name}` exports C symbols but is not named `*-ffi` — FFI export crates follow the `-ffi` naming convention"),
            ));
    }

    if !super::is_sys_crate(name)
        && let Some(line) = usage.foreign_block_line
    {
        out.push(violation(
                ctx.rel,
                line,
                format!("crate `{name}` links foreign C items but is not named `*-sys` — FFI import crates follow the `-sys` naming convention"),
            ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const CORE_REL: &str = "packages/core/src/lib.rs";

    fn run_at(rel: &str, source: &str) -> Vec<Violation> {
        crate::test_support::check_source_at(rel, source, check_ffi_crate_naming)
    }

    #[gtest]
    fn examples() -> Result<()> {
        for ex in EXAMPLES {
            let violations = run_at(CORE_REL, ex.code);

            verify_eq!(violations.is_empty(), ex.pass)?;
        }

        Ok(())
    }

    #[gtest]
    fn export_from_ffi_crate_passes() -> Result<()> {
        let source = "#[no_mangle]\npub extern \"C\" fn service_tick() {}";

        verify_true!(run_at("nested/core-ffi/src/lib.rs", source).is_empty())?;
        verify_true!(run_at("nested/core_ffi/src/lib.rs", source).is_empty())?;

        Ok(())
    }

    #[gtest]
    fn foreign_block_in_sys_crate_passes() -> Result<()> {
        let source = "extern \"C\" {\n    fn native_call();\n}";

        verify_true!(run_at("nested/native-sys/src/lib.rs", source).is_empty())?;
        verify_true!(run_at("nested/native_sys/src/lib.rs", source).is_empty())?;

        Ok(())
    }

    #[gtest]
    fn export_direction_flags_ffi_naming_only() -> Result<()> {
        let violations = run_at(
            "nested/core-sys/src/lib.rs",
            "#[no_mangle]\npub extern \"C\" fn f() {}",
        );

        verify_eq!(violations.len(), 1)?;

        Ok(())
    }

    #[gtest]
    fn files_without_an_owning_package_are_skipped() -> Result<()> {
        verify_true!(run_at("clean.rs", "extern \"C\" {\n    fn native_call();\n}").is_empty())?;

        Ok(())
    }

    #[gtest]
    fn patterns_inside_strings_do_not_count() -> Result<()> {
        let source = "const SNIPPET: &str = \"#[no_mangle] pub extern \\\"C\\\" fn f() {}\";";

        verify_true!(run_at(CORE_REL, source).is_empty())?;

        Ok(())
    }
}
