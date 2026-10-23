use crate::{Example, Violation, WorkspaceCtx, matches_ignore, violation};

const RULE: &str = "toml_cargo_layers";
const EXAMPLES: &[Example] = &[Example {
    label: "unrestricted dependency",
    code: "[dependencies]\nserde = \"1\"",
    pass: true,
}];

crate::language_workspace_rule!(
    toml_cargo_layers,
    "Reject dependency edges configured as manifest-glob !> package-glob,package-glob.",
    "Explicit dependency boundaries prevent accidental coupling between workspace layers. Configure the boundaries appropriate to your project; no package names are reserved.",
    High,
    default = false,
    full_manifest_graph = true,
    params {
        rules: [String] = []
    },
);

pub(crate) fn validate_edges(edges: &[String]) -> Result<(), String> {
    for edge in edges {
        let Some((from, targets)) = edge.split_once("!>") else {
            return Err(format!(
                "invalid layer edge `{edge}`: expected `manifest-glob !> package-glob,...`"
            ));
        };

        for pattern in std::iter::once(from).chain(targets.split(',')) {
            let pattern = pattern.trim();

            if pattern.is_empty() || pattern.contains("!>") {
                return Err(format!("invalid empty or nested layer pattern in `{edge}`"));
            }

            crate::glob::validate(pattern)?;
        }
    }

    Ok(())
}

fn check_toml_cargo_layers(ctx: &WorkspaceCtx<'_>) -> Vec<Violation> {
    let mut violations = Vec::new();

    for manifest in ctx.manifests {
        if !ctx.config.applies_to(RULE, &manifest.rel) {
            continue;
        }

        let edges = ctx
            .config
            .get_str_array(RULE, &TOML_CARGO_LAYERS_PARAMS[0], &manifest.rel);

        for edge in edges {
            let Some((from, targets)) = edge.split_once("!>") else {
                continue;
            };

            if !matches_ignore(&manifest.rel, &[from.trim()]) {
                continue;
            }

            let targets = targets.split(',').map(str::trim).collect::<Vec<_>>();

            for dependency in &manifest.dependencies {
                let inherited = (dependency.source == crate::DependencySource::Workspace)
                    .then(|| {
                        ctx.manifests
                            .iter()
                            .flat_map(|manifest| &manifest.workspace_dependencies)
                            .find(|candidate| candidate.name == dependency.name)
                    })
                    .flatten();
                let package = dependency
                    .package
                    .as_deref()
                    .or_else(|| inherited.and_then(|dep| dep.package.as_deref()))
                    .unwrap_or(&dependency.name);
                let target_manifest = ctx
                    .manifests
                    .iter()
                    .find(|target| target.package_name.as_deref() == Some(package));

                if matches_ignore(&dependency.name, &targets)
                    || matches_ignore(package, &targets)
                    || target_manifest.is_some_and(|target| matches_ignore(&target.rel, &targets))
                {
                    violations.push(violation(
                        &manifest.rel,
                        dependency.line,
                        format!(
                            "dependency `{}` (package `{package}`) violates layer edge `{edge}`",
                            dependency.name
                        ),
                    ));
                }
            }
        }
    }

    violations
}
