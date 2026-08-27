//! Cargo workspace discovery: root lookup, member enumeration, and dependency roots.

use std::sync::{Arc, Mutex};

use crate::{
    file,
    path::{Path, PathBuf},
    working_directory,
};
use std::collections::HashMap;

/// Workspace member package names and the crate roots they are imported as.
#[derive(Debug, Default)]
pub(crate) struct WorkspaceMembers {
    pub names: Vec<String>,
    pub import_roots: Vec<String>,
}

/// Per-run Cargo workspace discovery cache.
#[derive(Debug, Default)]
pub(crate) struct WorkspaceContext {
    external_roots: Mutex<HashMap<PathBuf, Arc<[String]>>>,
    members: Mutex<HashMap<PathBuf, Arc<WorkspaceMembers>>>,
}

impl WorkspaceContext {
    pub(crate) fn members(&self, start: &Path) -> Arc<WorkspaceMembers> {
        let root = workspace_root(start).unwrap_or_default();
        let mut cache = self
            .members
            .lock()
            .expect("workspace member cache poisoned");

        Arc::clone(
            cache
                .entry(root.clone())
                .or_insert_with(|| Arc::new(load_members(&root))),
        )
    }

    pub(crate) fn external_dep_roots(&self, path: &Path) -> Option<Arc<[String]>> {
        if !path.is_absolute() {
            return None;
        }

        let crate_dir = owning_crate_dir(path)?;
        let mut cache = self
            .external_roots
            .lock()
            .expect("dependency root cache poisoned");

        Some(Arc::clone(cache.entry(crate_dir.clone()).or_insert_with(
            || load_external_deps(&crate_dir, self).into(),
        )))
    }
}

impl WorkspaceMembers {
    pub(crate) fn is_member_root(&self, root: &str) -> bool {
        self.import_roots.iter().any(|known| known == root)
    }

    pub(crate) fn is_member_package(&self, name: &str) -> bool {
        self.names.iter().any(|known| known == name)
    }
}

/// Cargo workspace root for the package or workspace containing `start`.
pub(crate) fn workspace_root(start: &Path) -> Option<PathBuf> {
    let absolute = if start.is_absolute() {
        start.to_path_buf()
    } else {
        working_directory::current().ok()?.join(start)
    };

    let manifest = absolute
        .ancestors()
        .map(|dir| dir.join("Cargo.toml"))
        .find(|manifest| std::path::Path::new(manifest.as_os_str()).is_file())?;
    let mut command = cargo_metadata::MetadataCommand::new();

    command.manifest_path(&manifest).no_deps();

    command
        .exec()
        .ok()
        .map(|metadata| metadata.workspace_root.into_std_path_buf().into())
}

fn owning_crate_dir(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .skip(1)
        .find(|dir| {
            read_manifest(&dir.join("Cargo.toml"))
                .is_some_and(|document| document.contains_key("package"))
        })
        .map(Path::to_path_buf)
}

fn load_external_deps(crate_dir: &Path, workspace: &WorkspaceContext) -> Vec<String> {
    let Some(document) = read_manifest(&crate_dir.join("Cargo.toml")) else {
        return Vec::new();
    };
    let Some(dependencies) = document.get("dependencies").and_then(toml::Value::as_table) else {
        return Vec::new();
    };
    let members = workspace.members(crate_dir);
    let mut roots: Vec<String> = dependencies
        .iter()
        .filter(|(key, value)| {
            let resolved = value
                .as_table()
                .and_then(|spec| spec.get("package"))
                .and_then(toml::Value::as_str)
                .unwrap_or(key);

            !members.is_member_package(resolved)
        })
        .map(|(key, _)| key.replace('-', "_"))
        .collect();

    roots.sort();
    roots.dedup();

    roots
}

fn load_members(root: &Path) -> WorkspaceMembers {
    let mut members = WorkspaceMembers::default();
    let mut command = cargo_metadata::MetadataCommand::new();

    command
        .manifest_path(std::path::PathBuf::from(
            root.join("Cargo.toml").as_os_str(),
        ))
        .no_deps();
    let Ok(metadata) = command.exec() else {
        return members;
    };

    for package in metadata.packages.iter().filter(|package| {
        metadata
            .workspace_members
            .iter()
            .any(|member| member == &package.id)
    }) {
        let name = package.name.to_string();

        push_unique(&mut members.import_roots, name.replace('-', "_"));
        push_unique(&mut members.names, name);
    }

    members
}

fn read_manifest(manifest: &Path) -> Option<toml::Table> {
    let contents = file::read_text(manifest).ok()?;

    toml::from_str(&contents).ok()
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}
