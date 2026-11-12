use crate::{
    DependencyRecord, DependencySection, Example, Violation, WorkspaceCtx, WorkspaceManifest,
    matches_ignore, violation,
};

const RULE: &str = "toml_cargo_layers";
const EXAMPLES: &[Example] = &[Example {
    label: "unrestricted dependency",
    code: "[dependencies]\nserde = \"1\"",
    pass: true,
}];

crate::language_workspace_rule!(
    toml_cargo_layers,
    "Enforce dependency edges: `manifest-glob !> glob,...` forbids, `manifest-glob => glob,...` lists everything allowed.",
    "Explicit dependency boundaries prevent accidental coupling between workspace layers. A deny edge rejects matching dependencies; an allow edge makes its targets, together with those of every other allow edge matching the manifest, the complete set of dependencies the manifest may declare. A trailing `(normal, build, dev)` limits an edge to those dependency kinds. Configure the boundaries appropriate to your project; no package names are reserved.",
    High,
    default = false,
    full_manifest_graph = true,
    params {
        rules: [String] = []
    },
);

#[derive(Clone, Copy, Eq, PartialEq)]
enum EdgeKind {
    Deny,
    Allow,
}

struct Edge<'a> {
    text: &'a str,
    kind: EdgeKind,
    from: &'a str,
    targets: Vec<&'a str>,
    sections: Vec<DependencySection>,
}

impl Edge<'_> {
    fn applies_to(&self, section: DependencySection) -> bool {
        self.sections.is_empty() || self.sections.contains(&section)
    }
}

fn parse_edge(edge: &str) -> Result<Edge<'_>, String> {
    let (kind, (from, rest)) = if let Some(parts) = edge.split_once("!>") {
        (EdgeKind::Deny, parts)
    } else if let Some(parts) = edge.split_once("=>") {
        (EdgeKind::Allow, parts)
    } else {
        return Err(format!(
            "invalid layer edge `{edge}`: expected `manifest-glob !> glob,...` or `manifest-glob => glob,...`"
        ));
    };
    let rest = rest.trim();
    let (targets, sections) = match rest
        .strip_suffix(')')
        .and_then(|body| body.rsplit_once('('))
    {
        Some((targets, sections)) => (targets.trim(), parse_sections(edge, sections)?),
        None => (rest, Vec::new()),
    };
    let targets = if targets.is_empty() {
        Vec::new()
    } else {
        targets.split(',').map(str::trim).collect()
    };
    let from = from.trim();

    if kind == EdgeKind::Deny && targets.is_empty() {
        return Err(format!("layer edge `{edge}` forbids nothing"));
    }

    for pattern in std::iter::once(from).chain(targets.iter().copied()) {
        if pattern.is_empty() || pattern.contains("!>") || pattern.contains("=>") {
            return Err(format!("invalid empty or nested layer pattern in `{edge}`"));
        }

        crate::glob::validate(pattern)?;
    }

    Ok(Edge {
        text: edge,
        kind,
        from,
        targets,
        sections,
    })
}

fn parse_sections(edge: &str, sections: &str) -> Result<Vec<DependencySection>, String> {
    sections
        .split(',')
        .map(|section| match section.trim() {
            "normal" => Ok(DependencySection::Normal),

            "build" => Ok(DependencySection::Build),

            "dev" => Ok(DependencySection::Dev),

            other => Err(format!(
                "layer edge `{edge}` names unknown dependency kind `{other}`; use normal, build, or dev"
            )),
        })
        .collect()
}

pub(crate) fn validate_edges(edges: &[String]) -> Result<(), String> {
    edges.iter().try_for_each(|edge| parse_edge(edge).map(drop))
}

fn check_toml_cargo_layers(ctx: &WorkspaceCtx<'_>) -> Vec<Violation> {
    let mut violations = Vec::new();

    for manifest in ctx.manifests {
        if !ctx.config.applies_to(RULE, &manifest.rel) {
            continue;
        }

        let configured =
            ctx.config
                .get_str_array(RULE, &TOML_CARGO_LAYERS_PARAMS[0], &manifest.rel);
        let edges = configured
            .iter()
            .filter_map(|edge| parse_edge(edge).ok())
            .filter(|edge| matches_ignore(&manifest.rel, &[edge.from]))
            .collect::<Vec<_>>();

        if edges.is_empty() {
            continue;
        }

        for dependency in &manifest.dependencies {
            let identity = identity(ctx, dependency);
            let applicable = edges
                .iter()
                .filter(|edge| edge.applies_to(dependency.section))
                .collect::<Vec<_>>();

            for edge in applicable
                .iter()
                .filter(|edge| edge.kind == EdgeKind::Deny && identity.matches(&edge.targets))
            {
                violations.push(violation(
                    &manifest.rel,
                    dependency.line,
                    format!(
                        "dependency `{}` (package `{}`) violates layer edge `{}`",
                        dependency.name, identity.package, edge.text
                    ),
                ));
            }

            let allow = applicable
                .iter()
                .filter(|edge| edge.kind == EdgeKind::Allow)
                .collect::<Vec<_>>();

            if !allow.is_empty() && !allow.iter().any(|edge| identity.matches(&edge.targets)) {
                let listed = allow
                    .iter()
                    .map(|edge| format!("`{}`", edge.text))
                    .collect::<Vec<_>>()
                    .join(", ");

                violations.push(violation(
                    &manifest.rel,
                    dependency.line,
                    format!(
                        "dependency `{}` (package `{}`) is not allowed by layer edge {listed}",
                        dependency.name, identity.package
                    ),
                ));
            }
        }
    }

    violations
}

struct Identity<'a> {
    name: &'a str,
    package: &'a str,
    manifest: Option<&'a WorkspaceManifest>,
}

impl Identity<'_> {
    fn matches(&self, targets: &[&str]) -> bool {
        matches_ignore(self.name, targets)
            || matches_ignore(self.package, targets)
            || self
                .manifest
                .is_some_and(|target| matches_ignore(&target.rel, targets))
    }
}

fn identity<'a>(ctx: &WorkspaceCtx<'a>, dependency: &'a DependencyRecord) -> Identity<'a> {
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

    Identity {
        name: &dependency.name,
        package,
        manifest: ctx
            .manifests
            .iter()
            .find(|target| target.package_name.as_deref() == Some(package)),
    }
}
