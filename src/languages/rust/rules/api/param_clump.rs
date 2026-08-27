use std::collections::HashMap;

use crate::{
    Example, Violation,
    languages::workspace::{FunctionRecord, WorkspaceCtx},
    violation,
};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "three parameters travel across three functions",
        code: "fn one(user: String, region: u32, locale: String) {}\nfn two(user: String, region: u32, locale: String, extra: bool) {}\nfn three(user: String, region: u32, locale: String) {}",
        pass: false,
    },
    Example {
        label: "pass through strengthens evidence",
        code: "fn sink(user: String, region: u32, locale: String) {}\nfn middle(user: String, region: u32, locale: String) { sink(user, region, locale); }\nfn source(user: String, region: u32, locale: String) { middle(user, region, locale); }",
        pass: false,
    },
    Example {
        label: "trait impl methods are exempt",
        code: "trait Handle { fn one(&self, user: String, region: u32, locale: String); fn two(&self, user: String, region: u32, locale: String); fn three(&self, user: String, region: u32, locale: String); }\nstruct Handler; impl Handle for Handler { fn one(&self, user: String, region: u32, locale: String) {} fn two(&self, user: String, region: u32, locale: String) {} fn three(&self, user: String, region: u32, locale: String) {} }",
        pass: true,
    },
    Example {
        label: "only two functions",
        code: "fn one(user: String, region: u32, locale: String) {}\nfn two(user: String, region: u32, locale: String) {}",
        pass: true,
    },
];

crate::workspace_rule!(
    param_clump,
    "Find maximal parameter groups repeated across functions; full-workspace runs are authoritative.",
    "Parameters that repeatedly travel together usually represent one missing domain value object.",
    Low,
    params {
        min_clump: i64 = 3,
        min_fns: i64 = 3
    },
);

#[derive(Clone, Copy)]
struct Located<'a> {
    rel: &'a str,
    record: &'a FunctionRecord,
}

// #rw(fn: rust_clone_in_loop) inverted indexes own parameter signatures and member sets
fn check_param_clump(ctx: &WorkspaceCtx<'_>) -> Vec<Violation> {
    let min_clump = ctx.config.get_usize("rust_param_clump", &PARAMS[0]);
    let min_fns = ctx.config.get_usize("rust_param_clump", &PARAMS[1]);
    let functions: Vec<Located<'_>> = ctx
        .files
        .iter()
        .flat_map(|file| {
            file.functions.iter().map(|record| Located {
                rel: &file.rel,
                record,
            })
        })
        .collect();
    let mut inverted = HashMap::<(String, String), Vec<usize>>::default();

    for (index, function) in functions.iter().enumerate() {
        for parameter in &function.record.params {
            inverted.entry(parameter.clone()).or_default().push(index);
        }
    }

    let mut clumps = HashMap::<Vec<usize>, Vec<(String, String)>>::default();

    for (parameter, mut members) in inverted {
        members.sort_unstable();
        members.dedup();

        if members.len() >= min_fns {
            clumps.entry(members).or_default().push(parameter);
        }
    }

    clumps
        .into_iter()
        .filter_map(|(members, mut parameters)| {
            if parameters.len() < min_clump {
                return None;
            }

            parameters.sort_unstable();
            let mut located: Vec<Located<'_>> = members
                .iter()
                .filter_map(|index| functions.get(*index).copied())
                .collect();

            located.sort_by_key(|item| (item.rel, item.record.line));
            let anchor = *located.last()?;
            let fields = parameters
                .iter()
                .map(|(name, ty)| format!("{name}: {ty}"))
                .collect::<Vec<_>>()
                .join(", ");
            let member_list = located
                .iter()
                .map(|item| {
                    format!("{}:{} ({})", item.rel, item.record.line, item.record.name)
                })
                .collect::<Vec<_>>()
                .join(", ");
            let names: Vec<&str> = parameters.iter().map(|(name, _)| name.as_str()).collect();
            let pass_through = located
                .iter()
                .flat_map(|item| &item.record.pass_through_calls)
                .filter(|args| {
                    names
                        .iter()
                        .all(|name| args.iter().any(|arg| arg == name))
                })
                .count();

            Some(violation(
                anchor.rel,
                anchor.record.line,
                format!(
                    "parameter clump [{fields}] appears in {member_list}; {pass_through} pass-through call(s) reinforce it"
                ),
            ))
        })
        .collect()
}

crate::rulewright_workspace_test!(check_param_clump, {
    crate::example_tests!(EXAMPLES, check_param_clump);
});
