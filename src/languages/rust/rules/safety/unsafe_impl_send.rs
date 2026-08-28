use crate::{
    Example, FileCtx, Violation,
    infra::{helpers, parse, scanner},
    violation,
};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "unsafe impl Send without safety comment",
        code: "struct Foo(*mut u8);\nunsafe impl Send for Foo {}",
        pass: false,
    },
    Example {
        label: "unsafe impl Sync without safety comment",
        code: "struct Foo(*mut u8);\nunsafe impl Sync for Foo {}",
        pass: false,
    },
    Example {
        label: "unsafe impl Send with safety comment",
        code: "struct Foo(*mut u8);\n// SAFETY: the pointer is owned and never shared across threads\nunsafe impl Send for Foo {}",
        pass: true,
    },
    Example {
        label: "generic blanket impl flagged even with safety comment",
        code: "struct Wrapper<T>(T);\n// SAFETY: trust me\nunsafe impl<T> Send for Wrapper<T> {}",
        pass: false,
    },
    Example {
        label: "generic blanket Sync with bounds",
        code: "struct Wrapper<T>(T);\nunsafe impl<T: Copy> Sync for Wrapper<T> {}",
        pass: false,
    },
    Example {
        label: "other unsafe trait impl",
        code: "struct Chunk([u8; 8]);\n// SAFETY: all-zero bit pattern is valid\nunsafe impl Zeroable for Chunk {}",
        pass: true,
    },
    Example {
        label: "mention in string literal",
        code: r#"let s = "unsafe impl Send for Foo";"#,
        pass: true,
    },
    Example {
        label: "mention in comment",
        code: "// unsafe impl Send for Foo",
        pass: true,
    },
];

crate::line_rule!(
    unsafe_impl_send,
    "Flag `unsafe impl Send`/`Sync` without a `// SAFETY:` comment, and any generic (blanket) form.",
    "Bypassing Send/Sync bounds is the canonical unsoundness footgun, and a blanket impl over all T cannot be proven safe.",
    High,
);

const GENERIC_MSG: &str = "blanket `unsafe impl<..> Send/Sync` over a generic type is almost \
                           always unsound — implement for concrete types instead";
const MISSING_COMMENT_MSG: &str =
    "`unsafe impl Send/Sync` without a `// SAFETY:` comment on a preceding line";

fn check_unsafe_impl_send(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let mut out = Vec::new();

    for (index, line) in ctx.lines.iter().enumerate() {
        if parse::is_comment(line.trim()) {
            continue;
        }

        let code = scanner::code_only(line);
        let Some(generic) = find_unsafe_send_impl(&code) else {
            continue;
        };
        let lineno = index + 1;

        if generic {
            out.push(violation(ctx.rel, lineno, GENERIC_MSG));
        } else if !helpers::has_preceding_comment(ctx.lines, lineno, &["SAFETY:"]) {
            out.push(violation(ctx.rel, lineno, MISSING_COMMENT_MSG));
        }
    }

    out
}

fn find_unsafe_send_impl(code: &str) -> Option<bool> {
    let (_, after) = code.split_once("unsafe impl")?;
    let mut rest = after.trim_start();
    let generic = rest.starts_with('<');

    if generic {
        rest = skip_generics(rest)?;
    }

    let trait_name = ["Send", "Sync"].into_iter().find(|t| rest.starts_with(t))?;
    let tail = rest
        .strip_prefix(trait_name)?
        .trim_start()
        .strip_prefix("for")?;

    (tail.is_empty() || tail.starts_with(char::is_whitespace)).then_some(generic)
}

fn skip_generics(s: &str) -> Option<&str> {
    let mut depth: usize = 0;

    for (i, ch) in s.char_indices() {
        match ch {
            '<' => depth += 1,

            '>' => {
                depth = depth.checked_sub(1)?;

                if depth == 0 {
                    return s.get(i + 1..).map(str::trim_start);
                }
            }

            _ => {}
        }
    }

    None
}

crate::rulewright_test!(check_unsafe_impl_send, {
    crate::example_tests!(EXAMPLES, check_unsafe_impl_send);
});
