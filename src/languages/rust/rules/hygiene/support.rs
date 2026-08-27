use ra_ap_syntax::{AstNode, Edition, SourceFile, ast};

pub(super) fn parse_expr(source: &str) -> Option<ast::Expr> {
    let fixture = format!("fn fixture() {{ let value = {source}; }}");
    let parse = SourceFile::parse(&fixture, Edition::Edition2024);

    if !parse.errors().is_empty() {
        return None;
    }

    parse
        .tree()
        .syntax()
        .descendants()
        .find_map(ast::LetStmt::cast)?
        .initializer()
}
