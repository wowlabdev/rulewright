use rulewright::{Example, FileCtx, Violation};

const EXAMPLES: &[Example] = &[];

rulewright::pack_line_rule!(
    invalid_param_fixture,
    "Invalid fixture rule.",
    "Exercises the parameter diagnostic.",
    Low,
    params {
        enabled: bool = true
    },
);

fn check_invalid_param_fixture(_: &FileCtx<'_>) -> Vec<Violation> {
    Vec::new()
}

fn main() {}
