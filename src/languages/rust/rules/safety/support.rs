use ra_ap_syntax::ast;

// #rw(fn: rust_recursive_fn) syntax path qualifier depth is parser-bounded and naturally recursive
pub(super) fn path_segments(path: &ast::Path) -> Vec<String> {
    let mut segments = path
        .qualifier()
        .map_or_else(Vec::new, |qualifier| path_segments(&qualifier));

    if let Some(name) = path.segment().and_then(|segment| segment.name_ref()) {
        segments.push(name.text().to_string());
    }

    segments
}
