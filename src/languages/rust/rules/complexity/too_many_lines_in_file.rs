#[cfg(test)]
use googletest::prelude::*;

use crate::{Example, FileCtx, Violation, violation};

const PERCENT_DENOMINATOR: usize = 100;

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[];

crate::line_rule!(
    too_many_lines_in_file,
    "Flag files exceeding threshold lines.",
    "Files over 1500 lines are a sign that the module has too many responsibilities and should be split.",
    Medium,
    params {
        threshold: i64 = 1500,
        slack_percent: i64 = 20
    },
);

fn check_too_many_lines_in_file(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let max_lines = ctx
        .config
        .get_usize("rust_too_many_lines_in_file", &PARAMS[0]);
    let slack_percent = ctx
        .config
        .get_usize("rust_too_many_lines_in_file", &PARAMS[1]);
    let budget = ctx.lines.iter().find_map(|line| {
        let crate::infra::parse::DirectiveResult::Valid(directive) =
            crate::infra::parse::directive(line)?
        else {
            return None;
        };

        (directive.scope == crate::infra::parse::Scope::File)
            .then(|| directive.values.get("rust_too_many_lines_in_file").copied())
            .flatten()
    });
    let allowed = budget.map_or(max_lines, |budget| {
        budget.saturating_mul(PERCENT_DENOMINATOR + slack_percent) / PERCENT_DENOMINATOR
    });

    if ctx.lines.len() > allowed {
        let message = budget.map_or_else(
            || {
                format!(
                    "file has {} lines (max {max_lines}), consider splitting into smaller modules",
                    ctx.lines.len()
                )
            },
            |budget| {
                format!(
                    "file has {} lines, exceeding declared budget {budget} with {slack_percent}% slack (max {allowed})",
                    ctx.lines.len()
                )
            },
        );

        vec![violation(ctx.rel, 1, message)]
    } else {
        Vec::new()
    }
}

crate::rulewright_test!(check_too_many_lines_in_file, {
    #[gtest]
    fn too_many_lines_fails() -> Result<()> {
        let source = (0..1501)
            .map(|i| format!("// line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let v = run(&source);
        verify_eq!(v.len(), 1)?;
        verify_eq!(v[0].line, 1)?;
        verify_true!(v[0].message.contains("1501 lines"))?;

        Ok(())
    }

    #[gtest]
    fn exactly_max_passes() -> Result<()> {
        let source = (0..1500)
            .map(|i| format!("// line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        verify_true!(run(&source).is_empty())?;

        Ok(())
    }

    #[gtest]
    fn declared_budget_allows_slack() -> Result<()> {
        let mut lines =
            vec!["// #rw(file: rust_too_many_lines_in_file = 1300) cohesive table".to_owned()];
        lines.extend((1..=1559).map(|i| format!("// line {i}")));
        verify_true!(run(&lines.join("\n")).is_empty())?;

        Ok(())
    }

    #[gtest]
    fn declared_budget_reports_growth_beyond_slack() -> Result<()> {
        let mut lines =
            vec!["// #rw(file: rust_too_many_lines_in_file = 1300) cohesive table".to_owned()];
        lines.extend((1..=1560).map(|i| format!("// line {i}")));
        let violations = run(&lines.join("\n"));
        verify_eq!(violations.len(), 1)?;
        verify_true!(violations[0].message.contains("declared budget 1300"))?;

        Ok(())
    }
});
