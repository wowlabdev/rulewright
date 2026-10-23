//! Taplo-backed TOML parsing and rule execution.

pub(crate) mod rules;

use crate::{FileCtx, Rule, RuleCheck, RuleFix, Violation, languages::Analysis};

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

        if !file.config.applies_to(rule.info.name, file.rel) {
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

pub(crate) fn workspace_manifest(ctx: &TomlCtx<'_>) -> Option<crate::WorkspaceManifest> {
    if ctx.file.path.file_name()?.to_str()? != "Cargo.toml" || !ctx.parse.errors.is_empty() {
        return None;
    }

    let document = toml::from_str::<toml::Table>(ctx.file.contents).ok()?;
    let package_name = document
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned);
    let mut dependencies = Vec::new();

    extract_dependency_sections(ctx, &document, &ctx.dom.clone(), &mut dependencies);
    let mut workspace_dependencies = Vec::new();

    if let Some(workspace) = document.get("workspace").and_then(toml::Value::as_table) {
        extract_dependency_sections(
            ctx,
            workspace,
            &ctx.dom.get("workspace"),
            &mut workspace_dependencies,
        );
    }

    if let Some(targets) = document.get("target").and_then(toml::Value::as_table) {
        for (name, target) in targets {
            if let Some(target) = target.as_table() {
                extract_dependency_sections(
                    ctx,
                    target,
                    &ctx.dom.get("target").get(name.as_str()),
                    &mut dependencies,
                );
            }
        }
    }

    let mut patch_tables = std::collections::BTreeMap::new();

    if let Some(patches) = document.get("patch").and_then(toml::Value::as_table) {
        for (name, patch) in patches {
            if let Some(table) = patch.as_table() {
                patch_tables.insert(
                    name.clone(),
                    extract_dependencies(
                        ctx,
                        table,
                        &ctx.dom.get("patch").get(name.as_str()),
                        crate::DependencySection::Normal,
                    ),
                );
            }
        }
    }

    Some(crate::WorkspaceManifest {
        rel: ctx.file.rel.to_owned(),
        package_name,
        patch_tables,
        workspace_dependencies,
        dependencies,
    })
}

fn extract_dependency_sections(
    ctx: &TomlCtx<'_>,
    document: &toml::Table,
    dom: &taplo::dom::Node,
    out: &mut Vec<crate::DependencyRecord>,
) {
    for (name, section) in [
        ("dependencies", crate::DependencySection::Normal),
        ("dev-dependencies", crate::DependencySection::Dev),
        ("build-dependencies", crate::DependencySection::Build),
    ] {
        if let Some(table) = document.get(name).and_then(toml::Value::as_table) {
            out.extend(extract_dependencies(ctx, table, &dom.get(name), section));
        }
    }
}

fn extract_dependencies(
    ctx: &TomlCtx<'_>,
    table: &toml::Table,
    dom: &taplo::dom::Node,
    section: crate::DependencySection,
) -> Vec<crate::DependencyRecord> {
    table
        .iter()
        .map(|(name, value)| {
            let source = if value.get("workspace").and_then(toml::Value::as_bool) == Some(true) {
                crate::DependencySource::Workspace
            } else if let Some(path) = value.get("path").and_then(toml::Value::as_str) {
                crate::DependencySource::Path(path.to_owned())
            } else if value.get("git").is_some() {
                crate::DependencySource::Git
            } else {
                crate::DependencySource::Registry
            };
            let line = dom
                .get(name.as_str())
                .text_ranges(false)
                .next()
                .map_or(1, |range| {
                    ctx.line_of_offset(u32::from(range.start()) as usize)
                });

            crate::DependencyRecord {
                name: name.clone(),
                root: name.replace('-', "_"),
                line,
                section,
                source,
                package: value
                    .get("package")
                    .and_then(toml::Value::as_str)
                    .map(str::to_owned),
            }
        })
        .collect()
}
