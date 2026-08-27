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
        label: "arc dyn parameter",
        code: "pub fn start(db: Arc<dyn Database>) {}",
        pass: false,
    },
    Example {
        label: "box dyn parameter",
        code: "pub fn run(handler: Box<dyn Handler>) {}",
        pass: false,
    },
    Example {
        label: "rc dyn pub field",
        code: "pub struct App {\n    pub db: Rc<dyn Store>,\n}",
        pass: false,
    },
    Example {
        label: "nested arc dyn parameter",
        code: "pub fn start(db: Option<Arc<dyn Database>>) {}",
        pass: false,
    },
    Example {
        label: "box dyn return",
        code: "pub fn handler() -> Box<dyn Handler> { make() }",
        pass: false,
    },
    Example {
        label: "box dyn error return",
        code: "pub fn run() -> Result<(), Box<dyn std::error::Error>> { Ok(()) }",
        pass: true,
    },
    Example {
        label: "box dyn error send sync",
        code: "pub fn run() -> Box<dyn Error + Send + Sync> { make() }",
        pass: true,
    },
    Example {
        label: "private newtype wrapper",
        code: "pub struct DynStore(Arc<dyn Store>);",
        pass: true,
    },
    Example {
        label: "private fn with dyn wrapper",
        code: "fn wire(db: Arc<dyn Database>) {}",
        pass: true,
    },
    Example {
        label: "generic parameter instead",
        code: "pub fn start(db: impl Database) {}",
        pass: true,
    },
    Example {
        label: "dyn wrapper in test module",
        code: "#[cfg(test)]\nmod tests {\n    pub fn start(db: Arc<dyn Database>) {}\n}",
        pass: true,
    },
];

crate::ast_rule!(
    dyn_wrapper_in_api,
    "Flag `Rc<dyn …>`/`Arc<dyn …>`/`Box<dyn …>` in pub fn params, returns, and pub struct fields.",
    "Visible dyn wrappers lock APIs into object safety and infect user code; wrap trait objects in a private newtype.",
    Low,
);

const DYN_WRAPPERS: &[&str] = &["Arc", "Box", "Rc"];

fn check_dyn_wrapper_in_api(ctx: &AstCtx<'_>) -> Vec<Violation> {
    public_api_types(ctx)
        .flat_map(|ty| {
            let paths = ty
                .syntax()
                .descendants()
                .filter_map(ast::PathType::cast)
                .filter(|path_type| path_reachable_from(&ty, path_type));

            paths
                .filter_map(|path_type| dyn_wrapper(&path_type).map(|wrapper| (path_type, wrapper)))
                .map(|(path_type, wrapper)| {
                    ctx.violation(
                        &path_type,
                        format!("`{wrapper}<dyn …>` in pub API — hide the trait object behind a private newtype wrapper"),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn dyn_wrapper(path_type: &ast::PathType) -> Option<&'static str> {
    let segment = path_type.path()?.segment()?;
    let name_ref = segment.name_ref()?;
    let name = name_ref.text();
    let wrapper = DYN_WRAPPERS
        .iter()
        .find(|wrapper| *name == ***wrapper)
        .copied()?;
    let inner = segment
        .generic_arg_list()?
        .generic_args()
        .find_map(|arg| match arg {
            ast::GenericArg::TypeArg(arg) => arg.ty(),
            _ => None,
        })?;
    let ast::Type::DynTraitType(object) = inner else {
        return None;
    };

    (!is_error_object(&object)).then_some(wrapper)
}

/// `Box<dyn Error>` returns are the sanctioned error-object pattern.
fn is_error_object(object: &ast::DynTraitType) -> bool {
    object
        .syntax()
        .descendants()
        .filter_map(ast::NameRef::cast)
        .any(|name| name.text() == "Error")
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
        .filter(|field| {
            !ctx.is_in_test(field) && is_pub(field.visibility()) && inside_public_struct(field)
        })
        .filter_map(|field| field.ty());
    let tuples = ctx
        .nodes::<ast::TupleField>()
        .filter(|field| {
            !ctx.is_in_test(field) && is_pub(field.visibility()) && inside_public_struct(field)
        })
        .filter_map(|field| field.ty());

    functions.chain(fields).chain(tuples)
}

fn inside_public_struct<N>(field: &N) -> bool
where
    N: AstNode,
{
    field
        .syntax()
        .ancestors()
        .find_map(ast::Struct::cast)
        .is_some_and(|item| is_pub(item.visibility()))
}

fn is_pub(visibility: Option<ast::Visibility>) -> bool {
    visibility.is_some_and(|vis| matches!(vis.kind(), VisibilityKind::Pub))
}

crate::rulewright_ast_test!(check_dyn_wrapper_in_api, {
    crate::example_tests!(EXAMPLES, check_dyn_wrapper_in_api);

    #[gtest]
    fn pub_newtype_field_flagged() -> Result<()> {
        let v = run("pub struct DynStore(pub Arc<dyn Store>);");
        verify_eq!(v.len(), 1)?;

        Ok(())
    }
});
