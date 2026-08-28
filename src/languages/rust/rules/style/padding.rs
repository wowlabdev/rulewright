#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{AstNode, SyntaxNode, SyntaxToken, ast, syntax_editor::SyntaxEditor};

use crate::{AstCtx, Example, Violation};

use super::support::{gap_before_attached_comment, has_blank_line};

const CONTROL_FLOW: &str = "control-flow";
const FUNCTIONS: &str = "functions";
const LET_RUNS: &str = "let-runs";
const RETURNS: &str = "returns";
const TAIL_EXPRESSIONS: &str = "tail-expressions";
pub(crate) const VALID_BOUNDARIES: &[&str] =
    &[FUNCTIONS, CONTROL_FLOW, LET_RUNS, RETURNS, TAIL_EXPRESSIONS];

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "return has padding",
        code: "fn value(flag: bool) -> usize {\n    let value = 1;\n\n    return value;\n}",
        pass: true,
    },
    Example {
        label: "return needs padding",
        code: "fn value() -> usize {\n    let value = 1;\n    return value;\n}",
        pass: false,
    },
    Example {
        label: "first return is exempt",
        code: "fn value() -> usize {\n    return 1;\n}",
        pass: true,
    },
    Example {
        label: "tail expression has padding",
        code: "fn value() -> usize {\n    let value = 1;\n\n    value\n}",
        pass: true,
    },
    Example {
        label: "tail expression needs padding",
        code: "fn value() -> usize {\n    let value = 1;\n    value\n}",
        pass: false,
    },
    Example {
        label: "effect followed by tail expression stays compact",
        code: "fn value() -> Result<(), Error> {\n    verify_state()?;\n    finish()\n}",
        pass: true,
    },
    Example {
        label: "let run needs following padding",
        code: "fn run() {\n    let one = 1;\n    let two = 2;\n    consume(one, two);\n}",
        pass: false,
    },
    Example {
        label: "let run followed by padding",
        code: "fn run() {\n    let one = 1;\n    let two = 2;\n\n    consume(one, two);\n}",
        pass: true,
    },
    Example {
        label: "multiline control expression is padded",
        code: "fn run(flag: bool) {\n    prepare();\n\n    if flag {\n        work();\n    }\n\n    finish();\n}",
        pass: true,
    },
    Example {
        label: "multiline control expression needs padding before and after",
        code: "fn run(flag: bool) {\n    prepare();\n    if flag {\n        work();\n    }\n    finish();\n}",
        pass: false,
    },
    Example {
        label: "let after multiline guard needs padding",
        code: "fn inspect(path: &Path) {\n    if path.is_dir() {\n        return;\n    }\n    let manifest = path.join(\"Cargo.toml\");\n}",
        pass: false,
    },
    Example {
        label: "compact guard needs following padding",
        code: "fn inspect(path: &Path) {\n    if path.is_dir() { return; }\n    observe(path);\n}",
        pass: false,
    },
    Example {
        label: "compact consecutive guards need padding",
        code: "fn validate(one: bool, two: bool) {\n    if one { return; }\n    if two { return; }\n}",
        pass: false,
    },
    Example {
        label: "adjacent free functions need padding",
        code: "fn one() {}\nfn two() {}",
        pass: false,
    },
    Example {
        label: "separated free functions",
        code: "fn one() {}\n\nfn two() {}",
        pass: true,
    },
    Example {
        label: "adjacent methods need padding",
        code: "impl Value {\n    fn one() {}\n    fn two() {}\n}",
        pass: false,
    },
    Example {
        label: "adjacent trait methods need padding",
        code: "trait Value {\n    fn one();\n    fn two();\n}",
        pass: false,
    },
    Example {
        label: "else-if remains one control-flow chain",
        code: "fn choose(one: bool, two: bool) {\n    if one { work(); } else if two { work(); }\n}",
        pass: true,
    },
    Example {
        label: "first and last multiline controls need only interior padding",
        code: "fn run(flag: bool) {\n    if flag {\n        work();\n    }\n\n    loop {\n        break;\n    }\n}",
        pass: true,
    },
    Example {
        label: "block-like control expression separates let runs",
        code: "fn run(mut bits: u32) {\n    while bits != 0 {\n        let index = bits.trailing_zeros();\n\n        if index > 10 {\n            observe(index);\n        }\n\n        bits &= bits - 1;\n    }\n}",
        pass: true,
    },
    Example {
        label: "single-expression closure is exempt",
        code: "fn run() {\n    consume(|| { 1 });\n}",
        pass: true,
    },
    Example {
        label: "directive stays attached to multiline control",
        code: "fn run(flag: bool) {\n    let value = String::new();\n    // #rw(rust_panic) bounded control path\n    if flag {\n        consume(value.clone());\n    }\n}",
        pass: false,
    },
    Example {
        label: "directive stays attached to tail expression",
        code: "fn value() -> usize {\n    let value = 1;\n    // #rw(rust_panic) representative fixture\n    value\n}",
        pass: false,
    },
    Example {
        label: "block directive stays attached to loop",
        code: "fn run(values: &[String]) {\n    let mut copies = Vec::new();\n    // #rw(block: rust_panic) bounded fixture\n    for value in values {\n        copies.push(value.clone());\n    }\n}",
        pass: false,
    },
];

crate::ast_tree_rule!(
    padding,
    "Require configurable blank-line boundaries between functions and distinct statement groups.",
    "Consistent vertical separation makes functions, control flow, logical setup stages, and tail values easier to scan. Keep attributes, suppression directives, safety comments, and invariant explanations attached to the code they justify instead of inserting whitespace through those relationships.",
    Low,
    fix_padding,
    params {
        boundaries: [String] = [
            "functions",
            "control-flow",
            "let-runs",
            "returns",
            "tail-expressions",
        ] in [
            FUNCTIONS,
            CONTROL_FLOW,
            LET_RUNS,
            RETURNS,
            TAIL_EXPRESSIONS,
        ],
    },
);

#[derive(Clone)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "padding decisions combine four independent syntax classifications"
)]
struct Entry {
    syntax: SyntaxNode,
    is_let: bool,
    is_return: bool,
    is_control: bool,
    is_tail: bool,
}

fn check_padding(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let mut violations = Vec::new();
    let boundaries = ctx
        .file
        .config
        .get_str_array("rust_padding", &PADDING_PARAMS[0]);

    for list in ctx.nodes::<ast::StmtList>() {
        for entry in missing_gaps(&list, &boundaries) {
            violations.push(ctx.violation(
                entry.syntax.clone(),
                format!("blank line required before {}", entry_label(&entry)),
            ));
        }
    }

    if boundary_enabled(&boundaries, FUNCTIONS) {
        violations.extend(missing_function_gaps(ctx).into_iter().map(|function| {
            ctx.violation(
                &function,
                "consecutive function definitions need a blank line between them",
            )
        }));
    }

    violations
}

fn entries(list: &ast::StmtList) -> Vec<Entry> {
    let tail = list.tail_expr();

    list.syntax()
        .children()
        .filter_map(|child| {
            if let Some(statement) = ast::Stmt::cast(child.clone()) {
                return Some(entry_from_stmt(statement));
            }

            let expression = ast::Expr::cast(child)?;

            Some(Entry {
                is_control: is_control(&expression),
                is_return: matches!(expression, ast::Expr::ReturnExpr(_)),
                is_tail: tail
                    .as_ref()
                    .is_some_and(|tail| tail.syntax() == expression.syntax()),
                syntax: expression.syntax().clone(),
                is_let: false,
            })
        })
        .collect()
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "statement iterators yield owned facade nodes and this helper is used directly by Iterator::map"
)]
fn entry_from_stmt(statement: ast::Stmt) -> Entry {
    let is_let = matches!(statement, ast::Stmt::LetStmt(_));
    let expression = match &statement {
        ast::Stmt::ExprStmt(statement) => statement.expr(),
        _ => None,
    };

    Entry {
        syntax: statement.syntax().clone(),
        is_let,
        is_return: expression
            .as_ref()
            .is_some_and(|expr| matches!(expr, ast::Expr::ReturnExpr(_))),
        is_control: expression.as_ref().is_some_and(is_control),
        is_tail: false,
    }
}

fn is_control(expression: &ast::Expr) -> bool {
    matches!(
        expression,
        ast::Expr::IfExpr(_)
            | ast::Expr::MatchExpr(_)
            | ast::Expr::ForExpr(_)
            | ast::Expr::WhileExpr(_)
            | ast::Expr::LoopExpr(_)
    )
}

fn padding_gap_token(current: &SyntaxNode) -> Option<SyntaxToken> {
    gap_before_attached_comment(current)
}

fn missing_gaps(list: &ast::StmtList, boundaries: &[String]) -> Vec<Entry> {
    let entries = entries(list);

    entries
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(index, current)| {
            let previous = &entries[index - 1];
            let required = boundary_enabled(boundaries, RETURNS) && current.is_return
                || boundary_enabled(boundaries, TAIL_EXPRESSIONS)
                    && current.is_tail
                    && previous.is_let
                || boundary_enabled(boundaries, LET_RUNS) && previous.is_let && !current.is_let
                || boundary_enabled(boundaries, CONTROL_FLOW)
                    && (current.is_control || previous.is_control);

            required
                && padding_gap_token(&current.syntax)
                    .is_some_and(|padding_gap| !has_blank_line(&padding_gap))
        })
        .map(|(_, entry)| entry.clone())
        .collect()
}

fn missing_function_gaps(ctx: &AstCtx<'_>) -> Vec<ast::Fn> {
    ctx.nodes::<ast::Fn>()
        .filter_map(|current| {
            let next = current.syntax().next_sibling().and_then(ast::Fn::cast)?;

            function_gap_token(next.syntax())
                .is_some_and(|gap| !has_blank_line(&gap))
                .then_some(next)
        })
        .collect()
}

fn function_gap_token(current: &SyntaxNode) -> Option<SyntaxToken> {
    gap_before_attached_comment(current)
}

fn boundary_enabled(boundaries: &[String], boundary: &str) -> bool {
    debug_assert!(
        VALID_BOUNDARIES.contains(&boundary),
        "padding boundary must be declared in VALID_BOUNDARIES"
    );

    boundaries.iter().any(|configured| configured == boundary)
}

const fn entry_label(entry: &Entry) -> &'static str {
    if entry.is_return {
        "return statement"
    } else if entry.is_tail {
        "tail expression"
    } else if entry.is_control {
        "control-flow statement"
    } else {
        "statement following a let run"
    }
}

fn fix_padding(ctx: &AstCtx<'_>, _violations: &[Violation]) -> Option<String> {
    let (editor, root) = SyntaxEditor::with_ast_node(ctx.root);
    let boundaries = ctx
        .file
        .config
        .get_str_array("rust_padding", &PADDING_PARAMS[0]);
    let mut changed = false;

    for list in root.syntax().descendants().filter_map(ast::StmtList::cast) {
        for entry in missing_gaps(&list, &boundaries) {
            let token = padding_gap_token(&entry.syntax)?;

            editor.replace(
                token.clone(),
                editor.make().whitespace(&format!("\n{}", token.text())),
            );
            changed = true;
        }
    }

    if boundary_enabled(&boundaries, FUNCTIONS) {
        for function in root.syntax().descendants().filter_map(ast::Fn::cast) {
            let Some(next) = function.syntax().next_sibling().and_then(ast::Fn::cast) else {
                continue;
            };
            let Some(token) = function_gap_token(next.syntax()).filter(|gap| !has_blank_line(gap))
            else {
                continue;
            };

            editor.replace(
                token.clone(),
                editor.make().whitespace(&format!("\n{}", token.text())),
            );
            changed = true;
        }
    }

    changed.then(|| editor.finish().new_root().to_string())
}

crate::rulewright_ast_test!(check_padding, {
    crate::example_tests!(EXAMPLES, check_padding);
    crate::fix_tests!(EXAMPLES, ast_tree, check_padding, fix_padding);

    #[gtest]
    fn fix_places_padding_before_attached_comments() -> Result<()> {
        let cases = [
            (
                "fn run(flag: bool) {\n    let value = String::new();\n    // #rw(rust_panic) bounded control path\n    if flag {\n        consume(value.clone());\n    }\n}",
                "fn run(flag: bool) {\n    let value = String::new();\n\n    // #rw(rust_panic) bounded control path\n    if flag {\n        consume(value.clone());\n    }\n}",
            ),
            (
                "fn value() -> usize {\n    let value = 1;\n    // #rw(rust_panic) representative fixture\n    value\n}",
                "fn value() -> usize {\n    let value = 1;\n\n    // #rw(rust_panic) representative fixture\n    value\n}",
            ),
            (
                "fn run(values: &[String]) {\n    let mut copies = Vec::new();\n    // #rw(block: rust_panic) bounded fixture\n    for value in values {\n        copies.push(value.clone());\n    }\n}",
                "fn run(values: &[String]) {\n    let mut copies = Vec::new();\n\n    // #rw(block: rust_panic) bounded fixture\n    for value in values {\n        copies.push(value.clone());\n    }\n}",
            ),
            (
                "fn visit(points: &[Point]) {\n    let first = &points[0];\n    // BOUNDS: the empty case returned above, so index one starts the remaining points.\n    for point in &points[1..] {\n        consume(first, point);\n    }\n}",
                "fn visit(points: &[Point]) {\n    let first = &points[0];\n\n    // BOUNDS: the empty case returned above, so index one starts the remaining points.\n    for point in &points[1..] {\n        consume(first, point);\n    }\n}",
            ),
        ];

        for (source, expected) in cases {
            verify_eq!(
                crate::apply_ast_tree_fix(source, check_padding, fix_padding),
                expected
            )?;
        }

        Ok(())
    }

    #[gtest]
    fn every_multiline_control_kind_is_padded_from_ordinary_statements() -> Result<()> {
        let controls = [
            "if flag {\n        work();\n    }",
            "match flag {\n        true => work(),\n        false => work(),\n    }",
            "for value in values {\n        consume(value);\n    }",
            "while flag {\n        work();\n        break;\n    }",
            "loop {\n        work();\n        break;\n    }",
        ];

        for control in controls {
            let source = format!(
                "fn run(flag: bool, values: &[usize]) {{\n    prepare();\n    {control}\n    finish();\n}}"
            );
            let violations = run(&source);

            verify_eq!(violations.len(), 2)?;
        }

        Ok(())
    }

    #[gtest]
    fn configured_boundaries_can_disable_function_padding() -> Result<()> {
        let source = "fn one() {}\nfn two() {}";
        let mut config = crate::Config::generate_default(&[("rust_padding", PADDING_PARAMS)]);

        config
            .rules
            .get_mut("rust_padding")
            .or_fail()?
            .params
            .insert(
                "boundaries".to_owned(),
                toml::Value::Array(vec![toml::Value::String(CONTROL_FLOW.to_owned())]),
            );
        let lines: Vec<&str> = source.lines().collect();
        let file = crate::FileCtx {
            rel: "fixture.rs",
            path: crate::Path::new("fixture.rs"),
            package_name: None,
            package_publishable: None,
            lines: &lines,
            contents: source,
            config: &config,
        };
        let parse = ra_ap_syntax::SourceFile::parse(source, ra_ap_syntax::Edition::Edition2024);
        let root = parse.tree();
        let line_index = line_index::LineIndex::new(source);
        let ctx = AstCtx::new(&file, &root, &line_index, false);

        verify_true!(check_padding(&ctx).is_empty())
    }

    #[gtest]
    fn function_fix_keeps_an_attached_comment_with_the_following_function() -> Result<()> {
        let source = "fn one() {}\n// Explains two.\nfn two() {}";
        let expected = "fn one() {}\n\n// Explains two.\nfn two() {}";

        verify_eq!(
            crate::apply_ast_tree_fix(source, check_padding, fix_padding),
            expected
        )
    }

    #[gtest]
    fn function_fix_keeps_a_trailing_comment_with_the_previous_function() -> Result<()> {
        let source = "fn one() {} // Explains one.\nfn two() {}";
        let expected = "fn one() {} // Explains one.\n\nfn two() {}";
        let first = crate::apply_ast_tree_fix(source, check_padding, fix_padding);

        verify_eq!(first, expected)?;
        verify_eq!(
            crate::apply_ast_tree_fix(&first, check_padding, fix_padding),
            expected
        )
    }
});
