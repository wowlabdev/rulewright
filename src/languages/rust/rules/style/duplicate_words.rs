use winnow::token::take_until;

use crate::{Example, FileCtx, Fix, Violation, infra::parse, violation};

const ADJACENT_WORD_COUNT: usize = 2;

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "the the in doc comment",
        code: "/// the the value",
        pass: false,
    },
    Example {
        label: "normal doc comment",
        code: "/// the value",
        pass: true,
    },
    Example {
        label: "not a comment",
        code: "let the_the = 1;",
        pass: true,
    },
    Example {
        label: "is is in comment",
        code: "// is is",
        pass: false,
    },
    Example {
        label: "case insensitive duplicate",
        code: "/// The the value",
        pass: false,
    },
];

crate::line_rule!(
    duplicate_words,
    "Flag repeated words in comments like `the the` or `is is`.",
    "Repeated words like 'the the' are typos that slip past spell checkers and make documentation look sloppy.",
    Low,
    fix_duplicate_words,
);

fn check_duplicate_words(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let mut out = Vec::new();

    for (i, line) in ctx.lines.iter().enumerate() {
        let lineno = i + 1;
        let trimmed = line.trim();
        let Some(text) = parse::comment_content(trimmed) else {
            continue;
        };

        let words: Vec<&str> = text.split_whitespace().collect();

        for pair in words.windows(ADJACENT_WORD_COUNT) {
            let &[first, second] = pair else { continue };

            if first.eq_ignore_ascii_case(second) {
                out.push(violation(
                    ctx.rel,
                    lineno,
                    format!("duplicate word \"{}\" in comment", first.to_lowercase()),
                ));
                break;
            }
        }
    }

    out
}

fn fix_duplicate_words(ctx: &FileCtx<'_>, v: &Violation) -> Option<Fix> {
    let line = ctx.line(v.line)?;
    let trimmed = line.trim();
    let text = parse::comment_content(trimmed)?;

    let words: Vec<&str> = text.split_whitespace().collect();
    let pair = words.windows(ADJACENT_WORD_COUNT).find(|pair| {
        let [first, second] = *pair else {
            return false;
        };

        first.eq_ignore_ascii_case(second)
    })?;
    let &[first, second] = pair else { return None };
    let mut input = line;
    let before_word: &str = parse::try_parse(&mut input, take_until(0.., first))?;
    let after_first = before_word.len() + first.len();
    let remaining = line.get(after_first..)?;
    let mut rem_input = remaining;
    let gap: &str = parse::try_parse(&mut rem_input, take_until(0.., second))?;
    let remove_end = after_first + gap.len() + second.len();

    Some(Fix::replace_line(
        v.line,
        format!("{}{}", line.get(..after_first)?, line.get(remove_end..)?),
    ))
}

crate::rulewright_test!(check_duplicate_words, {
    crate::example_tests!(EXAMPLES, check_duplicate_words);
    crate::fix_tests!(line, check_duplicate_words, fix_duplicate_words);
});
