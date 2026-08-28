use crate::{
    Example, FileCtx, Violation,
    infra::{parse, scanner},
    violation,
};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "unnamed event",
        code: "event!(Level::INFO, foo = 1, \"msg\");",
        pass: false,
    },
    Example {
        label: "unnamed multi-line event",
        code: "event!(\n    Level::INFO,\n    file.path = p,\n);",
        pass: false,
    },
    Example {
        label: "name after level",
        code: "event!(Level::INFO, name: \"a.b.c\", \"msg\");",
        pass: false,
    },
    Example {
        label: "named event",
        code: "event!(name: \"file.open.success\", Level::INFO, \"msg\");",
        pass: true,
    },
    Example {
        label: "named multi-line event",
        code: "tracing::event!(\n    name: \"a.b.c\",\n    Level::INFO,\n);",
        pass: true,
    },
    Example {
        label: "event in comment",
        code: "// event!(Level::INFO, \"msg\");",
        pass: true,
    },
    Example {
        label: "event in string literal",
        code: "let s = \"event!(Level::INFO)\";",
        pass: true,
    },
    Example {
        label: "other macro named event",
        code: "custom_event!(Level::INFO, \"msg\");",
        pass: true,
    },
    Example {
        label: "non-event log macro",
        code: "info!(\"msg\");",
        pass: true,
    },
];

crate::line_rule!(
    log_named_events,
    "Flag `event!(...)` invocations without a `name:` argument before the level.",
    "Unnamed events cannot be grouped or filtered across log entries; name them `<component>.<operation>.<state>`.",
    Low,
);

const EVENT_OPEN: &str = "event!(";
const MAX_SCAN_LINES: usize = 30;

fn is_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn event_invocation_start(code: &str) -> Option<usize> {
    let mut from = 0;

    while let Some(pos) = code.get(from..).and_then(|rest| rest.find(EVENT_OPEN)) {
        let abs = from + pos;
        let prev = code.get(..abs).and_then(|before| before.chars().last());

        if !prev.is_some_and(is_ident_char) {
            return Some(abs + EVENT_OPEN.len());
        }

        from = abs + EVENT_OPEN.len();
    }

    None
}

fn consume(text: &str, depth: &mut usize, content: &mut String) -> bool {
    for ch in text.chars() {
        match ch {
            '(' => *depth += 1,

            ')' => {
                *depth -= 1;

                if *depth == 0 {
                    return true;
                }
            }

            _ => {}
        }

        if !ch.is_whitespace() {
            content.push(ch);
        }
    }

    false
}

fn has_name_before_level(content: &str) -> bool {
    let Some(name_pos) = content.find("name:") else {
        return false;
    };

    content
        .find("Level::")
        .is_none_or(|level_pos| name_pos < level_pos)
}

fn check_log_named_events(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let mut out = Vec::new();

    for (i, line) in ctx.lines.iter().enumerate() {
        if parse::is_comment(line.trim()) {
            continue;
        }

        let code = scanner::code_only(line);
        let Some(start) = event_invocation_start(&code) else {
            continue;
        };

        let mut content = String::new();
        let mut depth: usize = 1;
        let mut done = consume(code.get(start..).unwrap_or(""), &mut depth, &mut content);
        let mut next = i + 1;

        while !done && next - i < MAX_SCAN_LINES {
            let Some(next_line) = ctx.lines.get(next) else {
                break;
            };

            done = consume(&scanner::code_only(next_line), &mut depth, &mut content);
            next += 1;
        }

        if !has_name_before_level(&content) {
            out.push(violation(
                ctx.rel,
                i + 1,
                "event!() without a leading name: argument — name events `<component>.<operation>.<state>`",
            ));
        }
    }

    out
}

crate::rulewright_test!(check_log_named_events, {
    crate::example_tests!(EXAMPLES, check_log_named_events);
});
