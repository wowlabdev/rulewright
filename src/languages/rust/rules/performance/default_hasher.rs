use ra_ap_syntax::{ast, ast::HasGenericArgs};

use super::super::support::path_names;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "HashMap<K, V> field with default hasher",
        code: "struct S { m: HashMap<u32, u32> }",
        pass: false,
    },
    Example {
        label: "HashSet<T> parameter with default hasher",
        code: "fn f(s: HashSet<u32>) { drop(s); }",
        pass: false,
    },
    Example {
        label: "std HashMap::new constructor",
        code: "fn f() { let mut m = std::collections::HashMap::new(); m.insert(1, 1); }",
        pass: false,
    },
    Example {
        label: "HashSet::with_capacity constructor",
        code: "fn f() { let s: std::collections::HashSet<u64> = HashSet::with_capacity(8); drop(s); }",
        pass: false,
    },
    Example {
        label: "map with explicit hasher type param",
        code: "struct S { m: HashMap<u32, u32, FxBuildHasher> }",
        pass: true,
    },
    Example {
        label: "set with explicit hasher type param",
        code: "struct S { s: HashSet<u32, FxBuildHasher> }",
        pass: true,
    },
    Example {
        label: "fast-hasher crate alias",
        code: "fn f(m: foldhash::HashMap<u32, u32>) { drop(m); }",
        pass: true,
    },
    Example {
        label: "turbofish constructor with explicit hasher",
        code: "fn f() { let _m = HashMap::<u32, u32, FxBuildHasher>::new(); }",
        pass: true,
    },
    Example {
        label: "BTreeMap does not hash",
        code: "struct S { m: std::collections::BTreeMap<u32, u32> }",
        pass: true,
    },
    Example {
        label: "default hasher in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn t() { let _m: HashMap<u32, u32> = HashMap::new(); }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    default_hasher,
    "Flag std `HashMap`/`HashSet` types and constructors that use the default SipHash hasher.",
    "SipHash's DoS resistance may be unnecessary for trusted internal keys. Choose a faster hasher only after confirming keys cannot be attacker-controlled and hashing is material in the workload; otherwise keep the defensive default and scope or suppress the rule.",
);

fn check_default_hasher(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let type_violations = ctx
        .nodes::<ast::PathType>()
        .filter(|path| !ctx.is_in_test(path))
        .filter_map(|path_type| {
            let path = path_type.path()?;

            flags_type_path(&path).then(|| ctx.violation(&path_type, MSG_TYPE))
        });
    let constructor_violations = ctx
        .nodes::<ast::CallExpr>()
        .filter(|call| !ctx.is_in_test(call))
        .filter_map(|call| {
            let ast::Expr::PathExpr(function) = call.expr()? else {
                return None;
            };
            let path = function.path()?;

            flags_ctor_path(&path).then(|| ctx.violation(&function, MSG_CTOR))
        });

    type_violations.chain(constructor_violations).collect()
}

const FAST_HASH_MARKERS: &[&str] = &["ahash", "foldhash", "fxhash", "hashbrown", "rustc_hash"];
const MAP_KEY_VALUE_ARGS: usize = 2;
const SET_VALUE_ARGS: usize = 1;

const MSG_TYPE: &str = "default SipHash hasher — trusted internal keys should use a fast hasher (foldhash/FxHash) or an explicit hasher type param";
const MSG_CTOR: &str = "constructor builds a default-SipHash collection — use a fast hasher (foldhash/FxHash) for trusted internal keys";

fn default_hasher_args(segment: &ast::PathSegment) -> Option<usize> {
    let name_ref = segment.name_ref()?;
    let name = name_ref.text();

    if name == "HashMap" {
        Some(MAP_KEY_VALUE_ARGS)
    } else if name == "HashSet" {
        Some(SET_VALUE_ARGS)
    } else {
        None
    }
}

fn has_fast_marker(path: &ast::Path) -> bool {
    path_names(path)
        .iter()
        .any(|name| FAST_HASH_MARKERS.contains(&name.as_str()))
}

fn count_type_args(segment: &ast::PathSegment) -> Option<usize> {
    Some(
        segment
            .generic_arg_list()?
            .generic_args()
            .filter(|arg| matches!(arg, ast::GenericArg::TypeArg(_)))
            .count(),
    )
}

fn flags_type_path(path: &ast::Path) -> bool {
    let Some(last) = path.segment() else {
        return false;
    };
    let Some(expected) = default_hasher_args(&last) else {
        return false;
    };

    !has_fast_marker(path) && count_type_args(&last) == Some(expected)
}

fn flags_ctor_path(path: &ast::Path) -> bool {
    let Some(last) = path.segment() else {
        return false;
    };
    let Some(last_name) = last.name_ref().map(|name| name.text().to_string()) else {
        return false;
    };

    if last_name != "new" && last_name != "with_capacity" {
        return false;
    }

    let Some(ty_seg) = path.qualifier().and_then(|qualifier| qualifier.segment()) else {
        return false;
    };
    let Some(expected) = default_hasher_args(&ty_seg) else {
        return false;
    };

    if has_fast_marker(path) {
        return false;
    }

    count_type_args(&ty_seg).is_none_or(|n| n <= expected)
}

crate::rulewright_ast_test!(check_default_hasher, {
    crate::example_tests!(EXAMPLES, check_default_hasher);
});
