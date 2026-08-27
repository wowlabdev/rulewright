use ra_ap_syntax::{AstNode, AstToken, ast};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "panic with meaningful message (bug detection)",
        code: "fn f() { panic!(\"buffer len {} below header size\", 3); }",
        pass: true,
    },
    Example {
        label: "bare panic",
        code: "fn f() { panic!(); }",
        pass: false,
    },
    Example {
        label: "empty panic message",
        code: "fn f() { panic!(\"\"); }",
        pass: false,
    },
    Example {
        label: "unimplemented in production",
        code: "fn f() { unimplemented!(); }",
        pass: false,
    },
    Example {
        label: "todo in production",
        code: "fn f() { todo!(); }",
        pass: false,
    },
    Example {
        label: "panic in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn t() { panic!(); }\n}",
        pass: true,
    },
    Example {
        label: "no panic",
        code: "fn f() -> Result<(), String> { Ok(()) }",
        pass: true,
    },
    Example {
        label: "expect is allowed",
        code: "fn f() { Some(1).expect(\"always Some\"); }",
        pass: true,
    },
];

crate::ast_rule!(
    panic,
    "Ban `unimplemented!()`, `todo!()`, and message-less `panic!()` in library code.",
    "Detected programming bugs must panic with a message; todo!/unimplemented! mark unfinished code and message-less panics help nobody.",
    High,
);

fn check_panic(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::MacroCall>()
        .filter(|call| !ctx.is_in_test(call))
        .filter_map(|call| {
            let name = macro_name(&call)?;
            let message = if matches!(name.as_str(), "unimplemented" | "todo") {
                Some(format!(
                    "{name}!() in production code (finish the implementation or return Result)"
                ))
            } else if name == "panic" {
                panic_message_violation(&call).map(str::to_owned)
            } else {
                None
            }?;

            Some(ctx.violation(&call, message))
        })
        .collect()
}

fn macro_name(call: &ast::MacroCall) -> Option<String> {
    call.path()?
        .segment()?
        .name_ref()
        .map(|name| name.text().to_string())
}

fn panic_message_violation(call: &ast::MacroCall) -> Option<&'static str> {
    let tree = call.token_tree()?;
    let mut tokens = tree
        .syntax()
        .children_with_tokens()
        .filter_map(ra_ap_syntax::NodeOrToken::into_token)
        .filter(|token| !token.kind().is_trivia())
        .filter(|token| !matches!(token.text(), "(" | ")" | "[" | "]" | "{" | "}"));
    let Some(first) = tokens.next() else {
        return Some("panic!() without a message (state what bug was detected)");
    };

    if ast::String::cast(first).is_some_and(|string| {
        string
            .value()
            .is_ok_and(|message| message.trim().is_empty())
    }) {
        return Some("panic!(\"\") with an empty message (state what bug was detected)");
    }

    None
}

crate::rulewright_ast_test!(check_panic, {
    crate::example_tests!(EXAMPLES, check_panic);
});
