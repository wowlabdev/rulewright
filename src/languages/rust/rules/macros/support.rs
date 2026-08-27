use ra_ap_syntax::ast;

pub(super) fn is_quote_call(call: &ast::MacroCall) -> bool {
    let name = call
        .path()
        .and_then(|path| path.segment())
        .and_then(|segment| segment.name_ref());

    name.is_some_and(|name| matches!(name.text().as_str(), "quote" | "quote_spanned"))
}
