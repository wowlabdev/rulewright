//! Shared identifier-analysis helpers for the style naming rules.

use ra_ap_syntax::ast::{self, HasName};

/// Split a CamelCase identifier into words; an acronym run like `HTML` counts as one word.
pub(super) fn segments(name: &str) -> Vec<String> {
    let chars: Vec<char> = name.chars().collect();
    let mut words = Vec::new();
    let mut current = String::new();

    for (index, &c) in chars.iter().enumerate() {
        if c == '_' {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }

            continue;
        }

        let after_word_end = index
            .checked_sub(1)
            .and_then(|prev| chars.get(prev))
            .is_some_and(|prev| prev.is_lowercase() || prev.is_ascii_digit());
        let starts_new_word = chars.get(index + 1).is_some_and(char::is_ascii_lowercase);

        if c.is_uppercase() && !current.is_empty() && (after_word_end || starts_new_word) {
            words.push(std::mem::take(&mut current));
        }

        current.push(c);
    }

    if !current.is_empty() {
        words.push(current);
    }

    words
}

pub(super) fn type_def_name(item: &ast::Item) -> Option<ast::Name> {
    match item {
        ast::Item::Struct(item) => item.name(),
        ast::Item::Enum(item) => item.name(),
        ast::Item::Trait(item) => item.name(),
        ast::Item::TypeAlias(item) => item.name(),
        _ => None,
    }
}
