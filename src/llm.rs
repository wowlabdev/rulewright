// #rw(file: rust_alloc_in_loop) markdown document builder uses format! throughout loops by design
// #rw(file: rust_println) --llm CLI output writes the document to stdout by design
// #rw(file: rust_vec_string_field) rule-info structs are built then immediately rendered; boxing buys nothing

use std::{collections::BTreeMap, process::ExitCode};

use crate::markdown::{self, Doc, Table};

use crate::{
    Config, Example, ParamDefault, RuleKind, RuleParam, RuleRegistry, Severity, Violation, runner,
};

struct LlmRuleInfo {
    name: &'static str,
    kind: RuleKind,
    description: &'static str,
    justification: &'static str,
    severity: Severity,
    examples: &'static [Example],
    params: &'static [RuleParam],
    enabled: bool,
    fixable: bool,
    ignore_patterns: Vec<String>,
}

fn gather_rules(
    registry: &RuleRegistry,
    config: &Config,
    rule_filter: &[String],
) -> Vec<LlmRuleInfo> {
    registry
        .metadata()
        .into_iter()
        .filter(|r| rule_filter.is_empty() || rule_filter.iter().any(|f| f == r.name))
        .map(|r| LlmRuleInfo {
            name: r.name,
            kind: r.kind,
            description: r.description,
            justification: r.justification,
            severity: r.severity,
            examples: r.examples,
            params: r.params,
            enabled: config.is_enabled(r.name),
            fixable: r.fixable,
            ignore_patterns: config.ignore_patterns(r.name).to_vec(),
        })
        .collect()
}

fn render_summary_table(doc: Doc, rules: &[LlmRuleInfo]) -> Doc {
    let mut table = Table::new().headers([
        "Rule",
        "Severity",
        "Type",
        "Enabled",
        "Fixable",
        "Description",
    ]);

    for r in rules {
        let enabled = if r.enabled { "yes" } else { "no" };
        let fixable = if r.fixable { "yes" } else { "no" };

        table = table.row([
            r.name,
            r.severity.as_str(),
            r.kind.as_str(),
            enabled,
            fixable,
            r.description,
        ]);
    }

    doc.raw(table.build_markdown()).blank()
}

fn render_rule_details(mut doc: Doc, rules: &[LlmRuleInfo]) -> Doc {
    for r in rules {
        doc = doc.h3(r.name).blank().line(r.description).blank();

        if !r.justification.is_empty() {
            doc = doc.quote(r.justification).blank();
        }

        let enabled = if r.enabled { "yes" } else { "no" };
        let fixable = if r.fixable { "yes" } else { "no" };
        let mut meta = Table::new()
            .headers(["", ""])
            .row(["Severity", r.severity.as_str()])
            .row(["Type", r.kind.as_str()])
            .row(["Enabled", enabled])
            .row(["Fixable", fixable]);

        for p in r.params {
            let type_str = match p.param_type {
                crate::ParamType::Int => "i64",
                crate::ParamType::StringArray => "[String]",
            };
            let default_str = match &p.default {
                ParamDefault::Int(d) => d.to_string(),
                ParamDefault::StringArray(d) => format!("{d:?}"),
            };
            let label = format!("Param: {}", p.name);
            let value = format!("{type_str}, default = {default_str}");

            meta = meta.row([&label, &value]);
        }

        doc = doc.raw(meta.build_markdown());

        if !r.ignore_patterns.is_empty() {
            let patterns = r
                .ignore_patterns
                .iter()
                .map(markdown::code)
                .collect::<Vec<_>>()
                .join(", ");

            doc = doc.kv_bullet("Ignored paths", &patterns);
        }

        doc = doc.blank();

        let bad: Vec<_> = r.examples.iter().filter(|e| !e.pass).collect();
        let good: Vec<_> = r.examples.iter().filter(|e| e.pass).collect();

        if !bad.is_empty() {
            doc = doc
                .line(&markdown::bold("Bad (triggers violation):"))
                .blank();

            for ex in &bad {
                if !ex.label.is_empty() {
                    doc = doc.line(&markdown::italic(ex.label));
                }

                doc = doc.code_block("rust", ex.code).blank();
            }
        }

        if !good.is_empty() {
            doc = doc.line(&markdown::bold("Good (passes):")).blank();

            for ex in &good {
                if !ex.label.is_empty() {
                    doc = doc.line(&markdown::italic(ex.label));
                }

                doc = doc.code_block("rust", ex.code).blank();
            }
        }
    }

    doc
}

fn render_alignment_guide(doc: Doc) -> Doc {
    doc.h2("Alignment guide")
        .blank()
        .line("`// #rw:aligned` is not a suppression. It opts the immediately following table-like block into `rust_aligned`:")
        .blank()
        .code_block(
            "rust",
            "// #rw:aligned\nParser    => \"parser\";\nFormatter => \"formatter\";\nWriter    => \"writer\";",
        )
        .blank()
        .line("For blocks with at least two rows, Rulewright aligns `=>`, corresponding commas, and trailing `//` comments. The region ends at a blank line, a common closing delimiter, or another `#rw:` marker. Put the marker directly above the small block it describes; unmarked source is left alone.")
        .blank()
        .line("Array and slice tables can also keep each simple tuple on one line:")
        .blank()
        .code_block(
            "rust",
            "#[rustfmt::skip]\nlet cases = [\n    // #rw:aligned\n    (SHORT,     \"first\"),\n    (LONG_NAME, \"second\"),\n];",
        )
        .blank()
        .line("`rulewright --fix` can add missing padding and collapse wrapped tuple rows when every field is a literal or path. Calls, blocks, nested tuples, and other expressions are reported without an automatic rewrite. Rustfmt removes manual column padding, so use `#[rustfmt::skip]` on the containing item when the layout must survive `cargo fmt`.")
        .blank()
}

fn render_violations(mut doc: Doc, violations: &[Violation]) -> Doc {
    if violations.is_empty() {
        return doc.line("No violations found.").blank();
    }

    doc = doc
        .line(&markdown::bold(format!(
            "{} violation(s) found. Fix these:",
            violations.len()
        )))
        .blank();

    let mut by_rule: BTreeMap<&str, Vec<&Violation>> = BTreeMap::new();

    for v in violations {
        by_rule.entry(v.rule_name()).or_default().push(v);
    }

    for (rule, vs) in &by_rule {
        doc = doc.h3(&format!("{rule} ({} issues)", vs.len())).blank();

        for v in vs {
            let loc = format!("{}:{}", v.rel, v.line);

            doc = doc.def(&loc, &v.message);
        }

        doc = doc.blank();
    }

    doc
}

#[rustfmt::skip]
const DIRECTIVES: &[(&str, &str)] = &[
    // #rw:aligned
    ("// #rw(rule) reason",         "suppress the next line for this rule"),
    ("// #rw(rule1, rule2) reason", "suppress the next line for multiple rules"),
    ("// #rw(file: rule) reason",   "skip the entire file for this rule"),
    ("// #rw(block: rule) reason",  "suppress until blank line or closing brace"),
    ("// #rw(fn: rule) reason",     "suppress until end of next function"),
    ("// #rw(*) reason",            "suppress all rules (next line)"),
];

/// Render the rule catalog and current violations as a markdown document for LLM consumption.
#[must_use]
pub fn print(ctx: &runner::RunCtx<'_>) -> ExitCode {
    let rules = gather_rules(ctx.registry, ctx.config, ctx.rule_filter);

    let mut doc = Doc::new()
        .h1("rulewright — Rust source code linter")
        .blank()
        .line("Configured rules for the target Rust workspace. Runs beyond what clippy and rustfmt cover.")
        .blank();

    doc = render_alignment_guide(doc);

    let severity_table = Table::new()
        .headers(["Severity", "Meaning"])
        .row([
            "high",
            "Likely bug, security issue, or correctness problem. Fix these first.",
        ])
        .row([
            "medium",
            "Potential bug, performance issue, or maintainability concern.",
        ])
        .row([
            "low",
            "Style nit or minor improvement. Safe to suppress with good reason.",
        ]);

    doc = doc
        .h2("Severity levels")
        .blank()
        .raw(severity_table.build_markdown())
        .blank();

    doc = doc
        .h2("Suppression directives")
        .blank()
        .line("When a rule fires on code that is intentionally written that way, suppress it with a directive comment:")
        .blank();

    for (syntax, desc) in DIRECTIVES {
        doc = doc.def(syntax, desc);
    }

    doc = doc
        .blank()
        .line("Rule names and a reason after `)` are required. Missing either is a violation.")
        .blank();

    doc = doc.h2("Rules").blank();
    doc = render_summary_table(doc, &rules);

    doc = doc.h2("Rule details").blank();
    doc = render_rule_details(doc, &rules);

    doc = doc.h2("Current violations").blank();
    let violations = match runner::collect_violations(ctx, false, None) {
        Ok((violations, _, _, _)) => violations,
        Err(error) => {
            eprintln!("rulewright: analysis aborted: {error}");

            return ExitCode::FAILURE;
        }
    };

    doc = render_violations(doc, &violations);

    print!("{}", doc.build());

    ExitCode::SUCCESS
}
