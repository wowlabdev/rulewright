#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasVisibility},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "wildcard import",
        code: "use std::collections::*;",
        pass: false,
    },
    Example {
        label: "explicit import",
        code: "use std::collections::HashMap;",
        pass: true,
    },
    Example {
        label: "prelude allowed",
        code: "use my_crate::prelude::*;",
        pass: true,
    },
    Example {
        label: "super star allowed",
        code: "use super::*;",
        pass: true,
    },
    Example {
        label: "super nested allowed",
        code: "use super::types::*;",
        pass: true,
    },
    Example {
        label: "pub use reexport allowed",
        code: "pub use my_module::*;",
        pass: true,
    },
    Example {
        label: "pub crate reexport allowed",
        code: "pub(crate) use my_module::*;",
        pass: true,
    },
    Example {
        label: "private third party glob",
        code: "use some_crate::types::*;",
        pass: false,
    },
    Example {
        label: "enum glob in fn allowed",
        code: "fn f() { use MyEnum::*; }",
        pass: true,
    },
    Example {
        label: "enum glob at module level allowed",
        code: "use ColType::*;",
        pass: true,
    },
    Example {
        label: "crate internal types glob allowed",
        code: "use crate::types::*;",
        pass: true,
    },
    Example {
        label: "deep external glob",
        code: "use external_crate::deep::module::*;",
        pass: false,
    },
    Example {
        label: "wildcard in test module",
        code: "#[cfg(test)]\nmod tests {\n    use std::collections::*;\n}",
        pass: true,
    },
    Example {
        label: "nested prelude allowed",
        code: "use some_crate::module::prelude::*;",
        pass: true,
    },
];

crate::ast_rule!(
    wildcard_imports,
    "Ban `use foo::*` outside tests and preludes.",
    "Wildcard imports make it unclear where names come from and cause surprising breakage when upstream adds new items.",
    Medium,
);

fn check_wildcard_imports(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::Use>()
        .filter(|item| !ctx.is_in_test(item) && item.visibility().is_none())
        .flat_map(|item| {
            item.use_tree()
                .into_iter()
                .flat_map(|tree| tree.syntax().descendants().filter_map(ast::UseTree::cast))
                .filter_map(|tree| {
                    let star = tree.star_token()?;
                    let names = use_path_names(&tree);

                    should_flag_glob(&names).then(|| {
                        let line =
                            ctx.line_index.line_col(star.text_range().start()).line as usize + 1;

                        crate::violation(
                            ctx.file.rel,
                            line,
                            "wildcard import (use explicit imports)",
                        )
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn is_pascal_case(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    first.is_ascii_uppercase() && chars.any(|c| c.is_ascii_lowercase())
}

fn use_path_names(tree: &ast::UseTree) -> Vec<String> {
    let mut trees: Vec<ast::UseTree> = tree
        .syntax()
        .ancestors()
        .filter_map(ast::UseTree::cast)
        .collect();

    trees.reverse();

    trees
        .into_iter()
        .filter_map(|tree| tree.path())
        .flat_map(|path| {
            path.syntax()
                .descendants()
                .filter_map(ast::NameRef::cast)
                .map(|name| name.text().to_string())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn should_flag_glob(names: &[String]) -> bool {
    let parent = names.last().map(String::as_str);
    let grandparent = names.iter().rev().nth(1).map(String::as_str);

    !parent.is_some_and(|name| name == "prelude" || name == "super")
        && !grandparent.is_some_and(|name| name == "super" || name == "crate")
        && !parent.is_some_and(is_pascal_case)
}

crate::rulewright_ast_test!(check_wildcard_imports, {
    crate::example_tests!(EXAMPLES, check_wildcard_imports);

    #[gtest]
    fn production_caught_test_allowed() -> Result<()> {
        let v = run("use std::io::*;\n\
             #[cfg(test)]\n\
             mod tests {\n\
                 use std::collections::*;\n\
             }");
        verify_eq!(v.len(), 1)?;
        verify_eq!(v[0].line, 1)?;

        Ok(())
    }
});
