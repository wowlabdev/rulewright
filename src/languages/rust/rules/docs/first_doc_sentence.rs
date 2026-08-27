use crate::{Example, FileCtx, Violation, infra::parse, violation};

/// Pass/fail cases for `example_tests!`.
#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "short first sentence",
        code: "/// Returns the parsed value.\nfn f() {}",
        pass: true,
    },
    Example {
        label: "first sentence over the word budget",
        code: "/// This extremely long summary sentence keeps going and going with far too many words to fit the budget.",
        pass: false,
    },
    Example {
        label: "first sentence spills onto the next line",
        code: "/// This summary continues\n/// onto the next line.",
        pass: false,
    },
    Example {
        label: "long detail after a short summary",
        code: "/// Short summary.\n///\n/// Detail sentences after the summary can be as long as they want to be without limits.",
        pass: true,
    },
    Example {
        label: "inner doc short summary",
        code: "//! Module docs are covered.",
        pass: true,
    },
    Example {
        label: "inner doc summary spills",
        code: "//! Module summary that drifts\n//! across lines.",
        pass: false,
    },
    Example {
        label: "single line without terminator stays in budget",
        code: "/// Returns `None`",
        pass: true,
    },
    Example {
        label: "sentence ends before a code fence",
        code: "/// Sums things.\n/// ```\n/// let total = sum(items). More words inside fences never matter at all here\n/// ```",
        pass: true,
    },
    Example {
        label: "heading-only block is skipped",
        code: "/// # Safety",
        pass: true,
    },
    Example {
        label: "plain comments can ramble",
        code: "// this is not a doc comment so it can ramble on forever without any punctuation at all",
        pass: true,
    },
    Example {
        label: "empty first doc line delays the summary",
        code: "///\n/// The summary arrives too late here.",
        pass: false,
    },
    Example {
        label: "dotted names are not sentence ends",
        code: "/// Parses `foo.bar` fields quickly.",
        pass: true,
    },
];

crate::line_rule!(
    first_doc_sentence,
    "Require the first doc sentence to end on the first line within a word budget.",
    "The first sentence becomes the rustdoc summary; long or spilling summaries break skimmable docs.",
    Low,
    params {
        max_words: i64 = 15
    },
);

fn check_first_doc_sentence(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let max_words = ctx.config.get_usize("rust_first_doc_sentence", &PARAMS[0]);
    let mut out = Vec::new();

    for block in doc_blocks(ctx.lines) {
        check_block(ctx, &block, max_words, &mut out);
    }

    out
}

struct DocBlock<'a> {
    start_line: usize,
    lines: Vec<&'a str>,
}

fn doc_blocks<'a>(lines: &[&'a str]) -> Vec<DocBlock<'a>> {
    let mut blocks = Vec::new();
    let mut current: Option<DocBlock<'a>> = None;
    let mut in_fence = false;

    for (i, line) in lines.iter().enumerate() {
        if let Some(raw) = parse::doc_comment_content(line.trim()) {
            let text = raw.trim();
            let block = current.get_or_insert_with(|| DocBlock {
                start_line: i + 1,
                lines: Vec::new(),
            });

            if parse::matches(text, "```") {
                in_fence = !in_fence;
                continue;
            }

            if !in_fence {
                block.lines.push(text);
            }
        } else {
            if let Some(block) = current.take() {
                blocks.push(block);
            }

            in_fence = false;
        }
    }

    if let Some(block) = current.take() {
        blocks.push(block);
    }

    blocks
}

fn check_block(
    ctx: &FileCtx<'_>,
    block: &DocBlock<'_>,
    max_words: usize,
    out: &mut Vec<Violation>,
) {
    let Some(first) = block.lines.first().copied() else {
        return;
    };

    if first.is_empty() {
        if block.lines.iter().any(|line| !line.is_empty()) {
            out.push(violation(
                ctx.rel,
                block.start_line,
                "first doc sentence must start on the block's first line",
            ));
        }

        return;
    }

    if parse::matches(first, '#') {
        return;
    }

    let continues = block.lines.get(1).is_some_and(|line| !line.is_empty());

    match first_sentence_word_count(first) {
        Some(words) if words > max_words => out.push(violation(
            ctx.rel,
            block.start_line,
            format!("first doc sentence has {words} words (max {max_words})"),
        )),
        Some(_) => {}
        None if continues => out.push(violation(
            ctx.rel,
            block.start_line,
            "first doc sentence must end on the block's first line",
        )),
        None => {
            if first.split_whitespace().count() > max_words {
                out.push(violation(
                    ctx.rel,
                    block.start_line,
                    "first doc sentence exceeds the word budget",
                ));
            }
        }
    }
}

fn first_sentence_word_count(text: &str) -> Option<usize> {
    let mut words = 0;
    let mut in_word = false;
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch.is_whitespace() {
            in_word = false;
        } else if !in_word {
            in_word = true;
            words += 1;
        }

        if matches!(ch, '.' | '!' | '?') {
            match chars.peek() {
                None => return Some(words),
                Some(next) if next.is_whitespace() => return Some(words),
                Some(_) => {}
            }
        }
    }

    None
}

crate::rulewright_test!(check_first_doc_sentence, {
    crate::example_tests!(EXAMPLES, check_first_doc_sentence);
});
