use std::collections::HashSet;

use ra_ap_syntax::{
    AstNode, AstToken, Edition, SourceFile,
    ast::{self, HasAttrs, HasName},
};

use crate::path::{Path, PathBuf};

#[derive(Debug)]
struct ModuleEdge {
    source: String,
    target: String,
    requires_test: bool,
}

pub(crate) fn discover(
    paths: &[PathBuf],
    root: &Path,
    test_roots: &HashSet<String>,
) -> HashSet<String> {
    let known: HashSet<String> = paths
        .iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .filter_map(|path| relative(path, root))
        .collect();
    let mut edges = Vec::new();

    for path in paths
        .iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
    {
        let Some(source) = relative(path, root) else {
            continue;
        };
        let Ok(contents) = crate::file::read_text(path) else {
            continue;
        };
        let parsed = SourceFile::parse(&contents, Edition::Edition2024);

        if !parsed.errors().is_empty() {
            continue;
        }

        collect_module_edges(path, &source, root, &parsed.tree(), &known, &mut edges);
        collect_include_edges(path, &source, root, &parsed.tree(), &known, &mut edges);
    }

    let mut test_files = test_roots
        .iter()
        .filter(|path| known.contains(*path))
        .cloned()
        .collect::<HashSet<_>>();

    loop {
        let before = test_files.len();

        for edge in &edges {
            if edge.requires_test || test_files.contains(&edge.source) {
                test_files.insert(edge.target.clone());
            }
        }

        if test_files.len() == before {
            break;
        }
    }

    test_files
}

fn collect_module_edges(
    path: &Path,
    source: &str,
    workspace_root: &Path,
    syntax: &SourceFile,
    known: &HashSet<String>,
    edges: &mut Vec<ModuleEdge>,
) {
    for module in syntax
        .syntax()
        .descendants()
        .filter_map(ast::Module::cast)
        .filter(|module| module.semicolon_token().is_some())
    {
        let candidates = module_candidates(path, &module);
        let Some(target) = candidates
            .iter()
            .filter_map(|candidate| candidate_relative(candidate, workspace_root))
            .find(|candidate| known.contains(candidate))
        else {
            continue;
        };

        edges.push(ModuleEdge {
            source: source.to_owned(),
            target,
            requires_test: super::syntax_is_in_test(module.syntax()),
        });
    }
}

fn collect_include_edges(
    path: &Path,
    source: &str,
    workspace_root: &Path,
    syntax: &SourceFile,
    known: &HashSet<String>,
    edges: &mut Vec<ModuleEdge>,
) {
    for call in syntax
        .syntax()
        .descendants()
        .filter_map(ast::MacroCall::cast)
        .filter(|call| {
            call.path()
                .and_then(|path| path.as_single_name_ref())
                .is_some_and(|name| name.text() == "include")
        })
    {
        let Some(included) = call
            .token_tree()
            .and_then(|tree| {
                tree.syntax()
                    .descendants_with_tokens()
                    .filter_map(ra_ap_syntax::NodeOrToken::into_token)
                    .find_map(ast::String::cast)
            })
            .and_then(|literal| literal.value().ok().map(std::borrow::Cow::into_owned))
        else {
            continue;
        };
        let Some(parent) = path.parent() else {
            continue;
        };
        let candidate = parent.join(&included);
        let Some(target) = relative(&candidate, workspace_root) else {
            continue;
        };

        if known.contains(&target) {
            edges.push(ModuleEdge {
                source: source.to_owned(),
                target,
                requires_test: super::syntax_is_in_test(call.syntax()),
            });
        }
    }
}

fn module_candidates(path: &Path, module: &ast::Module) -> Vec<PathBuf> {
    if let Some(configured) = path_attribute(module) {
        return path_attribute_directory(path, module)
            .map(|directory| vec![directory.join(configured)])
            .unwrap_or_default();
    }

    let Some(module_dir) = module_directory(path, module) else {
        return Vec::new();
    };

    let Some(name) = module.name().map(|name| name.text().to_string()) else {
        return Vec::new();
    };

    vec![
        module_dir.join(format!("{name}.rs")),
        module_dir.join(name).join("mod.rs"),
    ]
}

fn module_directory(path: &Path, module: &ast::Module) -> Option<PathBuf> {
    let mut module_dir = path.parent()?.to_path_buf();
    let stem = path.file_stem().and_then(|stem| stem.to_str());

    if !matches!(stem, Some("lib" | "main" | "mod"))
        && let Some(stem) = stem
    {
        module_dir.push(stem);
    }

    for ancestor in inline_ancestors(module) {
        module_dir.push(ancestor);
    }

    Some(module_dir)
}

fn path_attribute_directory(path: &Path, module: &ast::Module) -> Option<PathBuf> {
    let mut directory = path.parent()?.to_path_buf();

    for ancestor in inline_ancestors(module) {
        directory.push(ancestor);
    }

    Some(directory)
}

fn inline_ancestors(module: &ast::Module) -> Vec<String> {
    let mut ancestors: Vec<String> = module
        .syntax()
        .ancestors()
        .skip(1)
        .filter_map(ast::Module::cast)
        .filter(|ancestor| ancestor.item_list().is_some())
        .filter_map(|ancestor| ancestor.name().map(|name| name.text().to_string()))
        .collect();

    ancestors.reverse();

    ancestors
}

fn path_attribute(module: &ast::Module) -> Option<String> {
    let meta = module
        .attrs()
        .find(|attribute| attribute.simple_name().as_deref() == Some("path"))?
        .meta()?;
    let ast::Meta::KeyValueMeta(meta) = meta else {
        return None;
    };
    let expression = meta.expr()?;
    let ast::Expr::Literal(literal) = expression else {
        return None;
    };
    let string = literal.syntax().first_token().and_then(ast::String::cast)?;

    string.value().ok().map(std::borrow::Cow::into_owned)
}

fn relative(path: &Path, root: &Path) -> Option<String> {
    path.strip_prefix(root).ok()?.to_slash()
}

fn candidate_relative(path: &Path, root: &Path) -> Option<String> {
    let native: &std::path::Path = path.as_ref();
    let mut normalized = std::path::PathBuf::new();

    for component in native.components() {
        match component {
            std::path::Component::CurDir => {}

            std::path::Component::ParentDir => {
                normalized.pop();
            }

            other => normalized.push(other.as_os_str()),
        }
    }

    let normalized = PathBuf::from(normalized);

    relative(&normalized, root)
}

#[cfg(test)]
mod tests {
    use googletest::prelude::*;

    use super::*;

    #[gtest]
    fn follows_cfg_test_path_modules_instead_of_filename_conventions() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let source = directory.path().join("src/lib.rs");
        let split = directory.path().join("src/checks.rs");

        crate::directory::ensure(source.parent().or_fail()?).or_fail()?;
        crate::file::write_text(
            &source,
            "#[cfg(test)]\n#[path = \"checks.rs\"]\nmod validation;\n",
        )
        .or_fail()?;
        crate::file::write_text(&split, "fn test_only() {}\n").or_fail()?;

        let discovered = discover(&[source, split], directory.path(), &HashSet::new());

        verify_eq!(discovered, HashSet::from(["src/checks.rs".to_owned()]))
    }

    #[gtest]
    fn propagates_test_only_classification_to_child_modules_and_includes() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let lib = directory.path().join("src/lib.rs");
        let split = directory.path().join("src/checks.rs");
        let child = directory.path().join("src/checks/cases.rs");
        let included = directory.path().join("src/inline_cases.rs");

        crate::directory::ensure(child.parent().or_fail()?).or_fail()?;
        crate::file::write_text(
            &lib,
            "#[cfg(test)]\n#[path = \"checks.rs\"]\nmod validation;\n\n#[cfg(test)]\nmod inline { include!(\"inline_cases.rs\"); }\n",
        )
        .or_fail()?;
        crate::file::write_text(&split, "mod cases;\n").or_fail()?;
        crate::file::write_text(&child, "fn child_test() {}\n").or_fail()?;
        crate::file::write_text(&included, "fn included_test() {}\n").or_fail()?;
        let paths = [lib, split, child, included];
        let discovered = discover(&paths, directory.path(), &HashSet::new());

        verify_eq!(
            discovered,
            HashSet::from([
                "src/checks.rs".to_owned(),
                "src/checks/cases.rs".to_owned(),
                "src/inline_cases.rs".to_owned(),
            ])
        )
    }

    #[gtest]
    fn path_attributes_follow_nested_module_directories_and_parent_segments() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let lib = directory.path().join("src/lib.rs");
        let nested = directory.path().join("src/fixtures/checks.rs");
        let shared = directory.path().join("src/shared.rs");

        crate::directory::ensure(nested.parent().or_fail()?).or_fail()?;
        crate::file::write_text(
            &lib,
            "#[cfg(test)]\nmod fixtures {\n    #[path = \"checks.rs\"]\n    mod checks;\n    #[path = \"../shared.rs\"]\n    mod shared;\n}\n",
        )
        .or_fail()?;
        crate::file::write_text(&nested, "fn nested_test() {}\n").or_fail()?;
        crate::file::write_text(&shared, "fn shared_test() {}\n").or_fail()?;
        let discovered = discover(&[lib, nested, shared], directory.path(), &HashSet::new());

        verify_eq!(
            discovered,
            HashSet::from([
                "src/fixtures/checks.rs".to_owned(),
                "src/shared.rs".to_owned(),
            ])
        )
    }

    #[gtest]
    fn path_attribute_in_external_module_is_relative_to_its_source_directory() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let lib = directory.path().join("src/lib.rs");
        let module = directory.path().join("src/navigation.rs");
        let split = directory.path().join("src/navigation_tests.rs");

        crate::directory::ensure(lib.parent().or_fail()?).or_fail()?;
        crate::file::write_text(&lib, "mod navigation;\n").or_fail()?;
        crate::file::write_text(
            &module,
            "#[cfg(test)]\n#[path = \"navigation_tests.rs\"]\nmod tests;\n",
        )
        .or_fail()?;
        crate::file::write_text(&split, "fn split_test() {}\n").or_fail()?;
        let discovered = discover(&[lib, module, split], directory.path(), &HashSet::new());

        verify_eq!(
            discovered,
            HashSet::from(["src/navigation_tests.rs".to_owned()])
        )
    }

    #[gtest]
    fn test_only_edges_follow_cfg_boolean_semantics() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let lib = directory.path().join("src/lib.rs");
        let test_only = directory.path().join("src/test_only.rs");
        let mixed = directory.path().join("src/mixed.rs");

        crate::directory::ensure(lib.parent().or_fail()?).or_fail()?;
        crate::file::write_text(
            &lib,
            "#[cfg(all(test, feature = \"fixtures\"))]\nmod test_only;\n\n#[cfg(any(test, feature = \"fixtures\"))]\nmod mixed;\n",
        )
        .or_fail()?;
        crate::file::write_text(&test_only, "fn test_only() {}\n").or_fail()?;
        crate::file::write_text(&mixed, "fn mixed() {}\n").or_fail()?;
        let discovered = discover(&[lib, test_only, mixed], directory.path(), &HashSet::new());

        verify_eq!(discovered, HashSet::from(["src/test_only.rs".to_owned()]))
    }

    #[gtest]
    fn cargo_test_roots_propagate_to_child_modules() -> Result<()> {
        let directory = crate::temporary::Directory::new().or_fail()?;
        let integration = directory.path().join("tests/api.rs");
        let support = directory.path().join("tests/api/support.rs");

        crate::directory::ensure(support.parent().or_fail()?).or_fail()?;
        crate::file::write_text(&integration, "mod support;\n").or_fail()?;
        crate::file::write_text(&support, "fn fixture() {}\n").or_fail()?;
        let paths = [integration, support];
        let roots = HashSet::from(["tests/api.rs".to_owned()]);
        let discovered = discover(&paths, directory.path(), &roots);

        verify_eq!(
            discovered,
            HashSet::from(["tests/api.rs".to_owned(), "tests/api/support.rs".to_owned(),])
        )
    }
}
