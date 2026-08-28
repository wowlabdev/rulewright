//! Taplo-backed TOML parsing and rule execution.

mod rules;

use crate::{FileCtx, Rule, RuleCheck, RuleFix, Violation, languages::Analysis, matches_ignore};

#[derive(Debug)]
pub struct TomlCtx<'a> {
    pub file: &'a FileCtx<'a>,
    pub parse: &'a taplo::parser::Parse,
    pub dom: &'a taplo::dom::Node,
}

impl TomlCtx<'_> {
    #[must_use]
    pub fn line_of_offset(&self, offset: usize) -> usize {
        self.file
            .contents
            .get(..offset.min(self.file.contents.len()))
            .unwrap_or(self.file.contents)
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            + 1
    }
}

pub(crate) fn analyze(file: &FileCtx<'_>, rules: &[&Rule], fix_mode: bool) -> Analysis {
    let parse = taplo::parser::parse(file.contents);
    let dom = parse.clone().into_dom();
    let ctx = TomlCtx {
        file,
        parse: &parse,
        dom: &dom,
    };
    let mut analysis = Analysis::default();

    if rules
        .iter()
        .any(|rule| matches!(rule.check, RuleCheck::Workspace(_)))
        && let Some(manifest) = workspace_manifest(&ctx)
    {
        analysis.workspace_manifests.push(manifest);
    }

    for rule in rules {
        let RuleCheck::Toml(check) = rule.check else {
            continue;
        };

        if matches_ignore(file.rel, file.config.ignore_patterns(rule.info.name)) {
            continue;
        }

        let mut violations: Vec<Violation> = check(&ctx)
            .into_iter()
            .map(|violation| violation.with_rule(rule.info.name))
            .collect();

        if let Some(RuleFix::Toml(fix)) = rule.fix {
            for violation in &mut violations {
                let Some(edit) = fix(&ctx, violation) else {
                    continue;
                };

                violation.mark_fixable();

                if fix_mode {
                    analysis.fixes.push((file.rel.to_owned(), edit));
                }
            }
        }

        analysis.violations.extend(violations);
    }

    analysis
}

fn workspace_manifest(ctx: &TomlCtx<'_>) -> Option<crate::languages::workspace::WorkspaceManifest> {
    if ctx.file.path.file_name()?.to_str()? != "Cargo.toml" || !ctx.parse.errors.is_empty() {
        return None;
    }

    let document = toml::from_str::<toml::Table>(ctx.file.contents).ok()?;

    if !document.contains_key("package") {
        return None;
    }

    let mut dependencies = Vec::new();

    extract_dependency_sections(ctx, &document, &mut dependencies);

    if let Some(targets) = document.get("target").and_then(toml::Value::as_table) {
        for target in targets.values().filter_map(toml::Value::as_table) {
            extract_dependency_sections(ctx, target, &mut dependencies);
        }
    }

    Some(crate::languages::workspace::WorkspaceManifest {
        rel: ctx.file.rel.to_owned(),
        dependencies,
    })
}

fn extract_dependency_sections(
    ctx: &TomlCtx<'_>,
    document: &toml::Table,
    out: &mut Vec<crate::languages::workspace::DependencyRecord>,
) {
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(table) = document.get(section).and_then(toml::Value::as_table) {
            extract_dependencies(ctx, section, table, out);
        }
    }
}

fn extract_dependencies(
    ctx: &TomlCtx<'_>,
    section: &str,
    table: &toml::Table,
    out: &mut Vec<crate::languages::workspace::DependencyRecord>,
) {
    for name in table.keys() {
        out.push(crate::languages::workspace::DependencyRecord {
            name: name.clone(),
            root: name.replace('-', "_"),
            line: dependency_line(ctx.file.lines, section, name),
        });
    }
}

fn dependency_line(lines: &[&str], section: &str, name: &str) -> usize {
    let mut in_section = false;

    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        if let Some(header) = trimmed
            .strip_prefix('[')
            .and_then(|header| header.strip_suffix(']'))
        {
            in_section = header == section || header.ends_with(&format!(".{section}"));
            continue;
        }

        if in_section
            && trimmed
                .split_once('=')
                .is_some_and(|(key, _)| key.trim().trim_matches('"') == name)
        {
            return index + 1;
        }
    }

    1
}
