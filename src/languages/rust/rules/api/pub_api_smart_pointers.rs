use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericArgs, HasVisibility, VisibilityKind},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "arc parameter",
        code: "pub fn process(data: Arc<Mutex<Shared>>) {}",
        pass: false,
    },
    Example {
        label: "box return",
        code: "pub fn build() -> Box<Processed> { Box::new(Processed) }",
        pass: false,
    },
    Example {
        label: "rc refcell parameter",
        code: "pub fn init(config: Rc<RefCell<Config>>) {}",
        pass: false,
    },
    Example {
        label: "pub wrapper field",
        code: "pub struct Server {\n    pub state: Arc<State>,\n}",
        pass: false,
    },
    Example {
        label: "pub method with wrapper return",
        code: "pub struct S;\nimpl S {\n    pub fn shared(&self) -> Rc<Data> { self.data.clone() }\n}",
        pass: false,
    },
    Example {
        label: "private fn with wrapper",
        code: "fn helper(data: Arc<Config>) {}",
        pass: true,
    },
    Example {
        label: "plain reference api",
        code: "pub fn process(data: &Data) -> State { data.state() }",
        pass: true,
    },
    Example {
        label: "box dyn left to dyn rule",
        code: "pub fn run(handler: Box<dyn Handler>) {}",
        pass: true,
    },
    Example {
        label: "boxed slice",
        code: "pub fn take(buf: Box<[u8]>) {}",
        pass: true,
    },
    Example {
        label: "boxed str",
        code: "pub fn name() -> Box<str> { String::new().into_boxed_str() }",
        pass: true,
    },
    Example {
        label: "private wrapper field",
        code: "pub struct Server {\n    state: Arc<State>,\n}",
        pass: true,
    },
    Example {
        label: "wrapper not outermost",
        code: "pub fn all(items: Vec<Arc<Config>>) {}",
        pass: true,
    },
    Example {
        label: "wrapper in test module",
        code: "#[cfg(test)]\nmod tests {\n    pub fn process(data: Arc<Mutex<Shared>>) {}\n}",
        pass: true,
    },
];

crate::ast_rule!(
    pub_api_smart_pointers,
    "Flag `Rc`/`Arc`/`Box`/`RefCell`/`Cell`/`Mutex`/`RwLock` as the outermost type of pub fn params, returns, and pub struct fields.",
    "Smart pointers in public APIs leak implementation details and infect downstream signatures; accept and return plain types.",
    Medium,
);

const WRAPPERS: &[&str] = &["Arc", "Box", "Cell", "Mutex", "Rc", "RefCell", "RwLock"];

fn check_pub_api_smart_pointers(ctx: &AstCtx<'_>) -> Vec<Violation> {
    public_api_types(ctx)
        .filter_map(|ty| {
            flagged_wrapper(&ty).map(|wrapper| {
                ctx.violation(
                    &ty,
                    format!(
                        "pub API exposes `{wrapper}` — accept/return plain types and keep wrappers internal"
                    ),
                )
            })
        })
        .collect()
}

fn flagged_wrapper(ty: &ast::Type) -> Option<&'static str> {
    let ast::Type::PathType(type_path) = ty else {
        return None;
    };
    let segment = type_path.path()?.segment()?;
    let name_ref = segment.name_ref()?;
    let name = name_ref.text();
    let wrapper = WRAPPERS
        .iter()
        .find(|wrapper| *name == ***wrapper)
        .copied()?;

    if let Some(inner) = first_type_arg(&segment) {
        if matches!(wrapper, "Arc" | "Box" | "Rc") && matches!(inner, ast::Type::DynTraitType(_)) {
            return None;
        }

        if wrapper == "Box" && is_dst(&inner) {
            return None;
        }
    }

    Some(wrapper)
}

fn first_type_arg(segment: &ast::PathSegment) -> Option<ast::Type> {
    segment
        .generic_arg_list()?
        .generic_args()
        .find_map(|arg| match arg {
            ast::GenericArg::TypeArg(arg) => arg.ty(),
            _ => None,
        })
}

/// `Box<[T]>` and `Box<str>` are idiomatic owned DSTs, not wrapper leakage.
fn is_dst(ty: &ast::Type) -> bool {
    match ty {
        ast::Type::SliceType(_) => true,

        ast::Type::PathType(path) => {
            let segment = path
                .path()
                .and_then(|path| path.segment())
                .and_then(|segment| segment.name_ref());

            segment.is_some_and(|name| name.text() == "str")
        }

        _ => false,
    }
}

// #rw(rust_similar_fns) Smart-pointer leakage intentionally scans public function and public-struct fields, while generic nesting also owns alias policy.
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
    let tuple_fields = ctx
        .nodes::<ast::TupleField>()
        .filter(|field| {
            !ctx.is_in_test(field) && is_pub(field.visibility()) && inside_public_struct(field)
        })
        .filter_map(|field| field.ty());

    functions.chain(fields).chain(tuple_fields)
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

crate::rulewright_ast_test!(check_pub_api_smart_pointers, {
    crate::example_tests!(EXAMPLES, check_pub_api_smart_pointers);
});
