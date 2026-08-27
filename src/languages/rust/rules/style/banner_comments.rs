use crate::{Example, FileCtx, Fix, Violation, infra::parse, violation};

const MIN_DECORATIVE_RUN: usize = 4;

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "plain separator",
        code: "// ----------------",
        pass: false,
    },
    Example {
        label: "framed banner",
        code: "// ==== parsing ====",
        pass: false,
    },
    Example {
        label: "mixed decorative separator",
        code: "// __~~__",
        pass: false,
    },
    Example {
        label: "ordinary comment",
        code: "// Parsing helpers live below.",
        pass: true,
    },
    Example {
        label: "short punctuation",
        code: "// --- note ---",
        pass: true,
    },
    Example {
        label: "comment-like string",
        code: "let banner = \"// --------\";",
        pass: true,
    },
];

crate::line_rule!(
    banner_comments,
    "Disallow decorative separator and framed banner comments.",
    "Structural code organization communicates sections more clearly than punctuation banners.",
    Low,
    fix_banner_comments,
);

fn check_banner_comments(ctx: &FileCtx<'_>) -> Vec<Violation> {
    ctx.lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim();

            if !parse::is_comment(trimmed)
                || trimmed.starts_with("///")
                || trimmed.starts_with("//!")
            {
                return None;
            }

            let content = trimmed.strip_prefix("//")?.trim();

            is_banner(content).then(|| violation(ctx.rel, index + 1, "decorative banner comment"))
        })
        .collect()
}

fn is_banner(content: &str) -> bool {
    let chars: Vec<char> = content.chars().collect();

    if chars.is_empty() {
        return false;
    }

    let only_decorative = chars
        .iter()
        .all(|ch| ch.is_whitespace() || is_decorative(*ch));
    let leading = chars.iter().take_while(|ch| is_decorative(**ch)).count();
    let trailing = chars
        .iter()
        .rev()
        .take_while(|ch| is_decorative(**ch))
        .count();

    (only_decorative && longest_run(&chars) >= MIN_DECORATIVE_RUN)
        || (leading >= MIN_DECORATIVE_RUN && trailing >= MIN_DECORATIVE_RUN)
}

fn is_decorative(ch: char) -> bool {
    matches!(ch, '-' | '=' | '*' | '#' | '~' | '_')
}

fn longest_run(chars: &[char]) -> usize {
    let mut longest = 0;
    let mut current = 0;

    for ch in chars {
        if is_decorative(*ch) {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }

    longest
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "line-rule fixer callbacks uniformly return Option<Fix>"
)]
fn fix_banner_comments(_ctx: &FileCtx<'_>, violation: &Violation) -> Option<Fix> {
    Some(Fix::delete(violation.line, violation.line))
}

crate::rulewright_test!(check_banner_comments, {
    crate::example_tests!(EXAMPLES, check_banner_comments);
    crate::fix_tests!(line, check_banner_comments, fix_banner_comments);
});
