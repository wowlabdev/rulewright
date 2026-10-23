use crate::{DependencySource, Example, Violation, WorkspaceCtx, matches_ignore, violation};

const RULE: &str = "toml_cargo_path_deps";
const EXAMPLES: &[Example] = &[
    Example {
        label: "registry dependency",
        code: "[dependencies]\nserde = \"1\"",
        pass: true,
    },
    Example {
        label: "outside path",
        code: "[dependencies]\nother = { path = \"../other\" }",
        pass: false,
    },
];

crate::language_workspace_rule!(
    toml_cargo_path_deps,
    "Restrict path dependency declarations and patches to approved manifests and workspace-local paths.",
    "Centralizing path declarations makes local dependency sources reviewable. Rejecting paths outside the workspace prevents builds from silently depending on sibling checkouts.",
    High,
    default = false,
    params {
        allow_in: [String] = ["Cargo.toml"],
        allow_outside_workspace: bool = false,
        allow_patch: bool = false,
    },
);

fn check_toml_cargo_path_deps(ctx: &WorkspaceCtx<'_>) -> Vec<Violation> {
    let mut violations = Vec::new();

    for manifest in ctx.manifests {
        let allow_in =
            ctx.config
                .get_str_array(RULE, &TOML_CARGO_PATH_DEPS_PARAMS[0], &manifest.rel);
        let outside = ctx
            .config
            .get_bool(RULE, &TOML_CARGO_PATH_DEPS_PARAMS[1], &manifest.rel);
        let allow_patches =
            ctx.config
                .get_bool(RULE, &TOML_CARGO_PATH_DEPS_PARAMS[2], &manifest.rel);
        let declarations = manifest
            .dependencies
            .iter()
            .chain(&manifest.workspace_dependencies)
            .map(|dep| (dep, false))
            .chain(
                manifest
                    .patch_tables
                    .values()
                    .flatten()
                    .map(|dep| (dep, true)),
            );

        for (dependency, is_patch) in declarations {
            let DependencySource::Path(path) = &dependency.source else {
                continue;
            };
            let reason = if is_patch && !allow_patches {
                Some("path patches are forbidden")
            } else if !matches_ignore(&manifest.rel, &allow_in) {
                Some("manifest is outside allow_in")
            } else if !outside && escapes_workspace(&manifest.rel, path) {
                Some("path escapes the workspace")
            } else {
                None
            };

            if let Some(reason) = reason {
                violations.push(violation(
                    &manifest.rel,
                    dependency.line,
                    format!("path dependency `{}` ({path}): {reason}", dependency.name),
                ));
            }
        }
    }

    violations
}

fn escapes_workspace(manifest: &str, path: &str) -> bool {
    let path = path.replace('\\', "/");

    if path.starts_with('/') || path.as_bytes().get(1) == Some(&b':') {
        return true;
    }

    let mut depth = manifest.split('/').count().saturating_sub(1);

    for component in path.split('/') {
        match component {
            "" | "." => {}

            ".." => {
                let Some(parent) = depth.checked_sub(1) else {
                    return true;
                };

                depth = parent;
            }

            _ => depth += 1,
        }
    }

    false
}
