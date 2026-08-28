use ra_ap_syntax::{
    AstNode,
    ast::{self, HasVisibility},
    syntax_editor::SyntaxEditor,
};

use crate::{AstCtx, Example, Violation};

const PUBLIC_METHOD_RANK: u8 = 2;
const RESTRICTED_METHOD_RANK: u8 = 3;
const PRIVATE_METHOD_RANK: u8 = 4;
const MACRO_CALL_RANK: u8 = 5;

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "canonical member groups",
        code: "struct Item;\nimpl Item {\n    const LIMIT: usize = 1;\n    type Value = usize;\n    pub fn new() -> Self { Self }\n    pub fn value(&self) {}\n    pub(crate) fn shared(&self) {}\n    fn private(&self) {}\n}",
        pass: true,
    },
    Example {
        label: "constructor follows method",
        code: "struct Item;\nimpl Item {\n    pub fn value(&self) {}\n    /// Builds an item.\n    #[cfg(test)]\n    pub fn new() -> Result<Self, ()> { Ok(Self) }\n}",
        pass: false,
    },
    Example {
        label: "restricted method before public method",
        code: "struct Item;\nimpl Item {\n    pub(crate) fn shared(&self) {}\n    #[inline]\n    pub fn visible(&self) {}\n}",
        pass: false,
    },
    Example {
        label: "trait impl is exempt",
        code: "struct Item;\nimpl Default for Item {\n    fn default() -> Self { Self }\n    const VALUE: usize = 1;\n}",
        pass: true,
    },
];

crate::ast_tree_rule!(
    impl_member_order,
    "Require inherent impl members to follow the canonical category and visibility order.",
    "Predictable inherent impl inventories keep construction and public APIs ahead of implementation details.",
    Medium,
    fix_impl_member_order,
);

fn check_impl_member_order(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::Impl>()
        .filter(|item_impl| item_impl.trait_().is_none())
        .filter_map(|item_impl| {
            let members: Vec<ast::AssocItem> = item_impl.assoc_item_list()?.assoc_items().collect();
            let ranks: Vec<u8> = members.iter().map(member_rank).collect();

            (!ranks.is_sorted()).then(|| {
                ctx.violation(
                    &item_impl,
                    "inherent impl members must be ordered as associated items, constructors, then methods by visibility",
                )
            })
        })
        .collect()
}

fn member_rank(member: &ast::AssocItem) -> u8 {
    match member {
        ast::AssocItem::Const(_) | ast::AssocItem::TypeAlias(_) => 0,
        ast::AssocItem::Fn(function) if is_constructor(function) => 1,
        ast::AssocItem::Fn(function) => visibility_rank(function),
        ast::AssocItem::MacroCall(_) => MACRO_CALL_RANK,
    }
}

fn is_constructor(function: &ast::Fn) -> bool {
    if function
        .param_list()
        .is_some_and(|params| params.self_param().is_some())
    {
        return false;
    }

    let Some(return_type) = function.ret_type().and_then(|ret| ret.ty()) else {
        return false;
    };
    let normalized: String = return_type
        .syntax()
        .text()
        .to_string()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect();

    normalized == "Self"
        || normalized.starts_with("Result<Self,")
        || normalized.starts_with("Option<Self>")
}

fn visibility_rank(function: &ast::Fn) -> u8 {
    match function.visibility() {
        None => PRIVATE_METHOD_RANK,

        Some(visibility) if visibility.syntax().text().to_string().trim() == "pub" => {
            PUBLIC_METHOD_RANK
        }

        Some(_) => RESTRICTED_METHOD_RANK,
    }
}

fn fix_impl_member_order(ctx: &AstCtx<'_>, _violations: &[Violation]) -> Option<String> {
    let (editor, root) = SyntaxEditor::with_ast_node(ctx.root);
    let mut changed = false;

    for item_impl in root.syntax().descendants().filter_map(ast::Impl::cast) {
        if item_impl.trait_().is_some() {
            continue;
        }

        let Some(list) = item_impl.assoc_item_list() else {
            continue;
        };
        let members: Vec<ast::AssocItem> = list.assoc_items().collect();
        let mut target = members.clone();

        target.sort_by_key(member_rank);

        if members == target {
            continue;
        }

        for (old, new) in members.iter().zip(&target) {
            editor.replace(old.syntax().clone(), new.syntax().clone_subtree());
        }

        changed = true;
    }

    changed.then(|| editor.finish().new_root().to_string())
}

crate::rulewright_ast_test!(check_impl_member_order, {
    crate::example_tests!(EXAMPLES, check_impl_member_order);
    crate::fix_tests!(
        EXAMPLES,
        ast_tree,
        check_impl_member_order,
        fix_impl_member_order
    );
});
