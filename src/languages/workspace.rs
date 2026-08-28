use crate::checksum::{self, Checksum};
use ra_ap_syntax::{
    AstNode, SyntaxKind,
    ast::{self, HasArgList, HasGenericParams, HasName},
};
use std::collections::HashSet;

use crate::{Config, FileCtx, infra::ignore::Suppressions};

#[derive(Debug)]
pub struct WorkspaceCtx<'a> {
    pub files: &'a [WorkspaceRustFile],
    pub manifests: &'a [WorkspaceManifest],
    pub config: &'a Config,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct WorkspaceRustFile {
    pub rel: String,
    pub structs: Vec<StructRecord>,
    pub functions: Vec<FunctionRecord>,
    pub strings: Vec<StringRecord>,
    pub crate_roots: HashSet<String>,
    pub(crate) suppressions: Suppressions,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct WorkspaceManifest {
    pub rel: String,
    pub dependencies: Vec<DependencyRecord>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct DependencyRecord {
    pub name: String,
    pub root: String,
    pub line: usize,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct StructRecord {
    pub name: String,
    pub line: usize,
    pub generics_arity: usize,
    pub fields: Vec<(String, String)>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct FunctionRecord {
    pub name: String,
    pub line: usize,
    pub body_token_count: usize,
    pub(crate) body_checksum: Checksum,
    pub body_shingles: Box<[ShingleFingerprint]>,
    pub params: Vec<(String, String)>,
    pub pass_through_calls: Box<[Box<[String]>]>,
}

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub struct ShingleFingerprint([u8; SHINGLE_FINGERPRINT_BYTES]);

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct StringRecord {
    pub value: String,
    pub line: usize,
}

const GENERATED_HEADER_LINES: usize = 8;
pub(crate) const FUNCTION_SHINGLE_SIZE: usize = 5;
const SHINGLE_FINGERPRINT_BYTES: usize = 16;
const QUALIFIED_PATH_TOKEN_COUNT: usize = 2;
const SPACED_QUALIFIED_PATH_TOKEN_COUNT: usize = 3;

fn empty_workspace_file(
    file: &FileCtx<'_>,
    crate_roots: HashSet<String>,
    suppressions: Suppressions,
) -> WorkspaceRustFile {
    WorkspaceRustFile {
        rel: file.rel.to_owned(),
        structs: Vec::new(),
        functions: Vec::new(),
        strings: Vec::new(),
        crate_roots,
        suppressions,
    }
}

// #rw(fn: rust_cyclomatic_complexity) one typed walk extracts the records requested by all workspace rules
pub(crate) fn extract(
    file: &FileCtx<'_>,
    root: &ast::SourceFile,
    suppressions: Suppressions,
    test_only_file: bool,
) -> WorkspaceRustFile {
    let crate_roots = crate_roots(root);

    if test_only_file
        || file
            .contents
            .lines()
            .take(GENERATED_HEADER_LINES)
            .any(|line| line.contains("@generated"))
    {
        return empty_workspace_file(file, crate_roots, suppressions);
    }

    let line_index = line_index::LineIndex::new(file.contents);
    let line_of = |node: &ra_ap_syntax::SyntaxNode| {
        line_index.line_col(node.text_range().start()).line as usize + 1
    };
    let structs = root
        .syntax()
        .descendants()
        .filter_map(ast::Struct::cast)
        .filter(|item| !in_test(item.syntax()))
        .filter_map(|item| {
            let fields = item.field_list()?;
            let ast::FieldList::RecordFieldList(fields) = fields else {
                return None;
            };
            let name = item.name()?;

            Some(StructRecord {
                name: name.text().to_string(),
                line: line_of(name.syntax()),
                generics_arity: item
                    .generic_param_list()
                    .map_or(0, |params| params.generic_params().count()),
                fields: fields
                    .fields()
                    .filter_map(|field| {
                        Some((
                            field.name()?.text().to_string(),
                            normalized(field.ty()?.syntax()),
                        ))
                    })
                    .collect(),
            })
        })
        .collect();
    let functions = root
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .filter(|function| !in_test(function.syntax()) && !in_trait_impl(function))
        .filter_map(|function| {
            let name_node = function.name()?;
            let name = name_node.text().to_string();
            let body = function.body()?;
            let params: Vec<(String, String)> = function
                .param_list()?
                .params()
                .filter_map(|param| {
                    let ast::Pat::IdentPat(pattern) = param.pat()? else {
                        return None;
                    };

                    Some((
                        pattern.name()?.text().to_string(),
                        normalized(param.ty()?.syntax()),
                    ))
                })
                .collect();
            let own_names: HashSet<&str> = params.iter().map(|(name, _)| name.as_str()).collect();
            let pass_through_calls: Vec<Box<[String]>> = body
                .syntax()
                .descendants()
                .filter_map(ast::CallExpr::cast)
                .filter_map(|call| {
                    let args: Vec<String> = call
                        .arg_list()?
                        .args()
                        .filter_map(|arg| {
                            let ast::Expr::PathExpr(path) = arg else {
                                return None;
                            };
                            let name = path.path()?.as_single_name_ref()?.text().to_string();

                            own_names.contains(name.as_str()).then_some(name)
                        })
                        .collect();

                    (!args.is_empty()).then(|| args.into_boxed_slice())
                })
                .collect();

            let (body_token_count, body_checksum, body_shingles) = body_summary(&body);

            Some(FunctionRecord {
                name,
                line: line_of(name_node.syntax()),
                body_token_count,
                body_checksum,
                body_shingles,
                params,
                pass_through_calls: pass_through_calls.into_boxed_slice(),
            })
        })
        .collect();
    let string_tokens = root
        .syntax()
        .descendants_with_tokens()
        .filter_map(ra_ap_syntax::NodeOrToken::into_token)
        .filter(|token| token.kind() == SyntaxKind::STRING);
    let strings = string_tokens
        .filter(|token| {
            let in_examples = token
                .parent_ancestors()
                .filter_map(ast::Const::cast)
                .any(|item| item.name().is_some_and(|name| name.text() == "EXAMPLES"));
            let in_attribute = token
                .parent_ancestors()
                .any(|node| ast::Attr::can_cast(node.kind()));
            let in_macro_lint_reason = is_macro_lint_reason(token);
            let in_rulewright_test = token
                .parent_ancestors()
                .filter_map(ast::MacroCall::cast)
                .filter_map(|call| call.path())
                .filter_map(|path| path.segment())
                .filter_map(|segment| segment.name_ref())
                .any(|name| {
                    let name = name.text();

                    name.starts_with("rulewright_") && name.contains("_test")
                });
            let in_test = token.parent().is_some_and(|parent| in_test(&parent));

            !in_examples
                && !in_attribute
                && !in_macro_lint_reason
                && !in_rulewright_test
                && !in_test
        })
        .map(|token| StringRecord {
            value: token.text().to_owned(),
            line: line_index.line_col(token.text_range().start()).line as usize + 1,
        })
        .collect();

    WorkspaceRustFile {
        rel: file.rel.to_owned(),
        structs,
        functions,
        strings,
        crate_roots,
        suppressions,
    }
}

fn is_macro_lint_reason(token: &ra_ap_syntax::SyntaxToken) -> bool {
    let Some(definition) = token.parent_ancestors().find_map(ast::MacroRules::cast) else {
        return false;
    };
    let mut previous = [None, None];

    for candidate in definition
        .syntax()
        .descendants_with_tokens()
        .filter_map(ra_ap_syntax::NodeOrToken::into_token)
        .filter(|candidate| !candidate.kind().is_trivia())
    {
        if candidate.text_range() == token.text_range() {
            return previous[0].as_deref() == Some("reason") && previous[1].as_deref() == Some("=");
        }

        previous = [previous[1].take(), Some(candidate.text().to_owned())];
    }

    false
}

impl ShingleFingerprint {
    fn from_tokens(tokens: &[String]) -> Self {
        let checksum = token_checksum(tokens);
        let mut fingerprint = [0_u8; SHINGLE_FINGERPRINT_BYTES];

        for (target, source) in fingerprint.iter_mut().zip(checksum.as_bytes()) {
            *target = *source;
        }

        Self(fingerprint)
    }
}

fn body_summary(body: &ast::BlockExpr) -> (usize, Checksum, Box<[ShingleFingerprint]>) {
    let body_tokens: Box<[String]> = body
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| {
            let token = element.into_token()?;

            (!token.kind().is_trivia()).then(|| token.text().to_owned())
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let checksum = token_checksum(&body_tokens);
    let mut shingles = body_tokens
        .windows(FUNCTION_SHINGLE_SIZE)
        .map(ShingleFingerprint::from_tokens)
        .collect::<Vec<_>>();

    shingles.sort_unstable();
    shingles.dedup();

    (body_tokens.len(), checksum, shingles.into_boxed_slice())
}

fn token_checksum(tokens: &[String]) -> Checksum {
    let byte_count = tokens
        .iter()
        .map(|token| token.len().saturating_add(size_of::<u64>()))
        .sum();
    let mut encoded = Vec::with_capacity(byte_count);

    for token in tokens {
        let len = u64::try_from(token.len()).unwrap_or(u64::MAX);

        encoded.extend_from_slice(&len.to_le_bytes());
        encoded.extend_from_slice(token.as_bytes());
    }

    checksum::bytes(encoded)
}

fn crate_roots(root: &ast::SourceFile) -> HashSet<String> {
    let mut roots: HashSet<String> = root
        .syntax()
        .descendants()
        .filter_map(ast::Path::cast)
        .filter(|path| path.qualifier().is_none())
        .filter_map(|path| {
            path.segment()?
                .name_ref()
                .map(|name| name.text().trim_start_matches("r#").to_owned())
        })
        .collect();

    roots.extend(
        root.syntax()
            .descendants()
            .filter_map(ast::ExternCrate::cast)
            .filter_map(|item| item.name_ref())
            .map(|name| name.text().trim_start_matches("r#").to_owned()),
    );

    let tokens: Vec<_> = root
        .syntax()
        .descendants_with_tokens()
        .filter_map(ra_ap_syntax::NodeOrToken::into_token)
        .filter(|token| !token.kind().is_trivia())
        .collect();

    for pair in tokens.windows(QUALIFIED_PATH_TOKEN_COUNT) {
        if pair[0].kind() == SyntaxKind::IDENT && pair[1].text() == "::" {
            roots.insert(pair[0].text().trim_start_matches("r#").to_owned());
        }
    }

    for triple in tokens.windows(SPACED_QUALIFIED_PATH_TOKEN_COUNT) {
        let [root, first_separator, second_separator] = triple else {
            continue;
        };

        if root.kind() == SyntaxKind::IDENT
            && first_separator.text() == ":"
            && second_separator.text() == ":"
        {
            roots.insert(root.text().trim_start_matches("r#").to_owned());
        }
    }

    roots
}

fn normalized(node: &ra_ap_syntax::SyntaxNode) -> String {
    node.descendants_with_tokens()
        .filter_map(ra_ap_syntax::NodeOrToken::into_token)
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.text().to_owned())
        .collect()
}

fn in_test(node: &ra_ap_syntax::SyntaxNode) -> bool {
    node.ancestors().filter_map(ast::Item::cast).any(|item| {
        use ra_ap_syntax::ast::HasAttrs;

        item.attrs()
            .any(|attr| attr.syntax().text().to_string().contains("cfg(test)"))
    })
}

fn in_trait_impl(function: &ast::Fn) -> bool {
    function
        .syntax()
        .ancestors()
        .find_map(ast::Impl::cast)
        .is_some_and(|item_impl| item_impl.trait_().is_some())
}
