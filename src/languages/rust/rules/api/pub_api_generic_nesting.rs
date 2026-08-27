#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericArgs, HasVisibility, VisibilityKind},
};

use super::support::path_reachable_from;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "two local generic levels in param",
        code: "pub fn serve(s: Service<Backend<Store>>) {}",
        pass: false,
    },
    Example {
        label: "two local generic levels in return",
        code: "pub fn build() -> Service<Backend<Store>> { make() }",
        pass: false,
    },
    Example {
        label: "two local generic levels in pub field",
        code: "pub struct App {\n    pub svc: Service<Backend<Store>>,\n}",
        pass: false,
    },
    Example {
        label: "two local generic levels in pub type alias",
        code: "pub type Handle = Service<Backend<Store>>;",
        pass: false,
    },
    Example {
        label: "crate-prefixed local generics",
        code: "pub fn serve(s: crate::Service<crate::Backend<Store>>) {}",
        pass: false,
    },
    Example {
        label: "local nesting under std container",
        code: "pub fn all() -> Vec<Service<Backend<Store>>> { Vec::new() }",
        pass: false,
    },
    Example {
        label: "std nesting",
        code: "pub fn buf(v: Vec<Vec<u8>>) {}",
        pass: true,
    },
    Example {
        label: "one local level",
        code: "pub fn serve(s: Service<Backend>) {}",
        pass: true,
    },
    Example {
        label: "local generic with std argument",
        code: "pub fn serve(s: Service<Vec<u8>>) {}",
        pass: true,
    },
    Example {
        label: "foreign generics",
        code: "pub fn spawn(t: tokio::task::JoinHandle<foo::Bar<Baz>>) {}",
        pass: true,
    },
    Example {
        label: "private fn nesting",
        code: "fn serve(s: Service<Backend<Store>>) {}",
        pass: true,
    },
    Example {
        label: "nesting in test module",
        code: "#[cfg(test)]\nmod tests {\n    pub fn serve(s: Service<Backend<Store>>) {}\n}",
        pass: true,
    },
];

crate::ast_rule!(
    pub_api_generic_nesting,
    "Flag pub fn signatures, pub struct fields, and pub type aliases nesting one local generic instantiation inside another (e.g. `Service<Backend<Store>>`).",
    "Nested crate-local generics infect user code with type parameters and trait bounds users never asked for; flatten or alias the composition.",
    Low,
);

const STD_CONTAINERS: &[&str] = &[
    "Arc",
    "BTreeMap",
    "BTreeSet",
    "BinaryHeap",
    "Box",
    "Cell",
    "Cow",
    "HashMap",
    "HashSet",
    "LinkedList",
    "Mutex",
    "Option",
    "PhantomData",
    "Pin",
    "Rc",
    "RefCell",
    "Result",
    "RwLock",
    "Vec",
    "VecDeque",
    "Weak",
];

fn check_pub_api_generic_nesting(ctx: &AstCtx<'_>) -> Vec<Violation> {
    public_api_types(ctx)
        .flat_map(|ty| {
            let paths = ty
                .syntax()
                .descendants()
                .filter_map(ast::PathType::cast)
                .filter(|path| path_reachable_from(&ty, path));
            let offending_paths = paths
                .filter_map(|path| offending_pair(&path).map(|pair| (path, pair)))
                .filter(|(path, _)| {
                    !path.syntax().ancestors().skip(1).filter_map(ast::PathType::cast).any(|ancestor| offending_pair(&ancestor).is_some())
                });

            offending_paths
                .map(|(outer_path, (outer, inner))| ctx.violation(
                    &outer_path,
                    format!(
                        "pub API nests local generic `{inner}<…>` inside `{outer}<…>` — flatten the abstraction or hide it behind an alias-free type"
                    ),
                ))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn local_generic_segment(path_type: &ast::PathType) -> Option<ast::PathSegment> {
    let path = path_type.path()?;
    let segments = path_segments(&path);
    let first_name = segments.first()?.name_ref()?;
    let first = first_name.text();
    let local = segments.len() == 1 || matches!(first.as_str(), "crate" | "self" | "super");

    if !local {
        return None;
    }

    let last = segments.last()?.clone();
    let name_ref = last.name_ref()?;
    let name = name_ref.text();

    if STD_CONTAINERS.contains(&name.as_str()) {
        return None;
    }

    last.generic_arg_list().is_some().then_some(last)
}

fn offending_pair(path_type: &ast::PathType) -> Option<(String, String)> {
    let outer = local_generic_segment(path_type)?;
    let inner = outer.generic_arg_list()?.generic_args().find_map(|arg| {
        let ast::GenericArg::TypeArg(arg) = arg else {
            return None;
        };
        let ast::Type::PathType(inner) = arg.ty()? else {
            return None;
        };

        local_generic_segment(&inner)
    })?;

    Some((
        outer.name_ref()?.text().to_string(),
        inner.name_ref()?.text().to_string(),
    ))
}

fn path_segments(path: &ast::Path) -> Vec<ast::PathSegment> {
    let mut reverse = Vec::new();
    let mut current = Some(path.clone());

    while let Some(path) = current {
        if let Some(segment) = path.segment() {
            reverse.push(segment);
        }

        current = path.qualifier();
    }

    reverse.into_iter().rev().collect()
}

fn public_api_types<'a>(ctx: &'a AstCtx<'a>) -> impl Iterator<Item = ast::Type> + 'a {
    let functions = ctx
        .nodes::<ast::Fn>()
        .filter(|function| !ctx.is_in_test(function) && is_pub(function.visibility()))
        .flat_map(|function| {
            let params = function
                .param_list()
                .into_iter()
                .flat_map(|params| params.params())
                .filter_map(|param| param.ty());

            params
                .chain(function.ret_type().and_then(|ret| ret.ty()))
                .collect::<Vec<_>>()
        });
    let fields = ctx
        .nodes::<ast::RecordField>()
        .filter(|field| !ctx.is_in_test(field) && is_pub(field.visibility()))
        .filter_map(|field| field.ty());
    let tuples = ctx
        .nodes::<ast::TupleField>()
        .filter(|field| !ctx.is_in_test(field) && is_pub(field.visibility()))
        .filter_map(|field| field.ty());
    let aliases = ctx
        .nodes::<ast::TypeAlias>()
        .filter(|alias| !ctx.is_in_test(alias) && is_pub(alias.visibility()))
        .filter_map(|alias| alias.ty());

    functions.chain(fields).chain(tuples).chain(aliases)
}

fn is_pub(visibility: Option<ast::Visibility>) -> bool {
    visibility.is_some_and(|vis| matches!(vis.kind(), VisibilityKind::Pub))
}

crate::rulewright_ast_test!(check_pub_api_generic_nesting, {
    crate::example_tests!(EXAMPLES, check_pub_api_generic_nesting);

    #[gtest]
    fn one_violation_per_nesting_site() -> Result<()> {
        let v = run("pub fn serve(s: Service<Backend<Store<Inner>>>) {}");
        verify_eq!(v.len(), 1)?;

        Ok(())
    }
});
