use crate::{Example, FileCtx, Violation, infra::parse, violation};

/// Pass/fail cases for `example_tests!`.
#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "local re-export without doc(inline)",
        code: "pub use crate::foo::Bar;",
        pass: false,
    },
    Example {
        label: "local re-export with doc(inline)",
        code: "#[doc(inline)]\npub use crate::foo::Bar;",
        pass: true,
    },
    Example {
        label: "local re-export with doc(hidden)",
        code: "#[doc(hidden)]\npub use crate::internal::Bar;",
        pass: true,
    },
    Example {
        label: "self re-export without doc(inline)",
        code: "pub use self::foo::Bar;",
        pass: false,
    },
    Example {
        label: "super re-export without doc(inline)",
        code: "pub use super::foo::Bar;",
        pass: false,
    },
    Example {
        label: "same-line attribute on local re-export",
        code: "#[doc(inline)] pub use crate::foo::Bar;",
        pass: true,
    },
    Example {
        label: "inlined std re-export",
        code: "#[doc(inline)]\npub use std::fmt::Debug;",
        pass: false,
    },
    Example {
        label: "same-line inlined core re-export",
        code: "#[doc(inline)] pub use core::fmt::Debug;",
        pass: false,
    },
    Example {
        label: "std re-export without inline is correct",
        code: "pub use std::fmt::Debug;",
        pass: true,
    },
    Example {
        label: "non-pub use is not a re-export",
        code: "use crate::foo::Bar;",
        pass: true,
    },
    Example {
        label: "commented-out re-export",
        code: "// pub use crate::foo::Bar;",
        pass: true,
    },
];

crate::line_rule!(
    doc_inline_reexport,
    "Require `#[doc(inline)]` on local re-exports and forbid it on external ones.",
    "Local re-exports should inline into module docs while external items stay visibly external.",
    Low,
);

/// Re-export path prefixes that point at items of this crate.
const LOCAL_PREFIXES: &[&str] = &["pub use crate::", "pub use self::", "pub use super::"];
/// Re-export path prefixes that point at external items.
const EXTERNAL_PREFIXES: &[&str] = &["pub use std::", "pub use core::", "pub use alloc::"];
/// Attribute that inlines a re-export into rustdoc.
const DOC_INLINE: &str = "#[doc(inline)]";
/// Attribute that hides a re-export from rustdoc.
const DOC_HIDDEN: &str = "#[doc(hidden)]";

fn check_doc_inline_reexport(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let mut out = Vec::new();

    for (i, line) in ctx.lines.iter().enumerate() {
        let trimmed = line.trim();

        if parse::is_comment(trimmed) {
            continue;
        }

        if LOCAL_PREFIXES.iter().any(|p| trimmed.starts_with(p)) && !has_doc_attr_above(ctx, i) {
            out.push(violation(
                ctx.rel,
                i + 1,
                "mark local re-exports with `#[doc(inline)]` (or `#[doc(hidden)]`)",
            ));
        }

        if let Some(rest) = trimmed.strip_prefix(DOC_INLINE)
            && inlines_external(ctx, i, rest)
        {
            out.push(violation(
                ctx.rel,
                i + 1,
                "do not `#[doc(inline)]` re-exports of external items",
            ));
        }
    }

    out
}

fn has_doc_attr_above(ctx: &FileCtx<'_>, index: usize) -> bool {
    let previous_line = index
        .checked_sub(1)
        .and_then(|prev| ctx.lines.get(prev))
        .map(|line| line.trim());

    previous_line.is_some_and(|prev| prev.starts_with(DOC_INLINE) || prev.starts_with(DOC_HIDDEN))
}

fn inlines_external(ctx: &FileCtx<'_>, index: usize, rest: &str) -> bool {
    let same_line = rest.trim_start();

    if EXTERNAL_PREFIXES.iter().any(|p| same_line.starts_with(p)) {
        return true;
    }

    ctx.lines
        .get(index + 1)
        .map(|line| line.trim())
        .is_some_and(|next| EXTERNAL_PREFIXES.iter().any(|p| next.starts_with(p)))
}

crate::rulewright_test!(check_doc_inline_reexport, {
    crate::example_tests!(EXAMPLES, check_doc_inline_reexport);
});
