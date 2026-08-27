use crate::path::Path;
#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::ast::{self, HasVisibility, VisibilityKind};

use super::naming;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "pub struct repeats module name",
        code: "pub struct FooId;",
        pass: false,
    },
    Example {
        label: "pub enum repeats module name",
        code: "pub enum FooKind { A }",
        pass: false,
    },
    Example {
        label: "pub trait repeats module name",
        code: "pub trait FooLike {}",
        pass: false,
    },
    Example {
        label: "pub type alias repeats module name",
        code: "pub type FooResult = ();",
        pass: false,
    },
    Example {
        label: "name equal to module name",
        code: "pub struct Foo;",
        pass: true,
    },
    Example {
        label: "prefix is not a whole segment",
        code: "pub struct Food;",
        pass: true,
    },
    Example {
        label: "unrelated name",
        code: "pub struct Bar;",
        pass: true,
    },
    Example {
        label: "private item exempt",
        code: "struct FooId;",
        pass: true,
    },
    Example {
        label: "prefixed type in test module",
        code: "#[cfg(test)]\nmod tests {\n    pub struct FooFixture;\n}",
        pass: true,
    },
];

crate::ast_rule!(
    module_prefix_in_name,
    "Flag pub type definitions whose name repeats the module name as a prefix (`FooId` in `foo.rs`).",
    "Module-qualified APIs can avoid repeating the module in every public type. Flat re-exports and collision-prone APIs are intentional exceptions because this syntax-only rule cannot inspect the final exported namespace.",
    Low,
    default = false,
);

fn check_module_prefix_in_name(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let Some(stem_parts) = stem_parts(ctx.file.rel) else {
        return Vec::new();
    };

    ctx.nodes::<ast::Item>()
        .filter(|item| !ctx.is_in_test(item) && is_pub(item))
        .filter_map(|item| {
            let name = naming::type_def_name(&item)?;
            let name_text = name.text();
            let segments = naming::segments(&name_text);
            let is_prefixed = segments.len() > stem_parts.len()
                && stem_parts
                    .iter()
                    .zip(&segments)
                    .all(|(part, segment)| part.eq_ignore_ascii_case(segment));

            is_prefixed.then(|| {
                ctx.violation(
                    &name,
                    format!(
                        "type name `{name_text}` repeats its module name — shorten only when callers use the module-qualified path; keep it when a flat re-export needs disambiguation"
                    ),
                )
            })
        })
        .collect()
}

fn stem_parts(rel: &str) -> Option<Vec<String>> {
    let path = Path::new(rel);
    let mut stem = path.file_stem()?.to_str()?;

    if stem == "mod" {
        stem = path.parent()?.file_name()?.to_str()?;
    }

    if stem == "lib" || stem == "main" {
        return None;
    }

    Some(
        stem.split('_')
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

fn is_pub(item: &ast::Item) -> bool {
    let visibility = match item {
        ast::Item::Struct(item) => item.visibility(),
        ast::Item::Enum(item) => item.visibility(),
        ast::Item::Trait(item) => item.visibility(),
        ast::Item::TypeAlias(item) => item.visibility(),
        _ => return false,
    };

    visibility.is_some_and(|visibility| matches!(visibility.kind(), VisibilityKind::Pub))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_at(rel: &str, source: &str) -> Vec<Violation> {
        crate::test_support::check_source_ast_at(rel, source, check_module_prefix_in_name)
    }

    #[gtest]
    fn examples() -> Result<()> {
        for ex in EXAMPLES {
            let violations = run_at("src/foo.rs", ex.code);

            verify_eq!(violations.is_empty(), ex.pass)?;
        }

        Ok(())
    }

    #[gtest]
    fn mod_rs_uses_parent_directory_name() -> Result<()> {
        verify_false!(run_at("src/widgets/mod.rs", "pub struct WidgetsId;").is_empty())?;
        verify_true!(run_at("src/widgets/mod.rs", "pub struct Widgets;").is_empty())?;

        Ok(())
    }

    #[gtest]
    fn snake_case_module_names_match_segment_wise() -> Result<()> {
        verify_false!(run_at("src/request_id.rs", "pub struct RequestIdMap;").is_empty())?;
        verify_true!(run_at("src/request_id.rs", "pub struct RequestId;").is_empty())?;

        Ok(())
    }

    #[gtest]
    fn lib_and_main_are_exempt() -> Result<()> {
        verify_true!(run_at("src/lib.rs", "pub struct LibId;").is_empty())?;
        verify_true!(run_at("src/main.rs", "pub struct MainLoop;").is_empty())?;

        Ok(())
    }
}
