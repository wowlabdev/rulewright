//! Cargo manifest rules.

mod edition;
mod feature_names;
mod feature_no_std;
mod msrv;
mod unused_deps;
mod workspace_dep_features;
mod workspace_lints;

use crate::TomlCtx;

const CARGO_WORKSPACE_REL: &str = "Cargo.toml";

fn is_cargo_member(rel: &str) -> bool {
    rel != "Cargo.toml" && is_cargo_manifest(rel)
}

fn is_cargo_manifest(rel: &str) -> bool {
    rel.rsplit('/').next() == Some("Cargo.toml")
}

fn assignment_line(lines: &[&str], key: &str) -> usize {
    let quoted = format!("\"{key}\"");

    lines
        .iter()
        .position(|line| {
            let trimmed = line.trim_start();

            trimmed
                .strip_prefix(&quoted)
                .or_else(|| trimmed.strip_prefix(key))
                .is_some_and(|rest| rest.trim_start().starts_with('='))
        })
        .map_or(1, |index| index + 1)
}

fn cargo_document(ctx: &TomlCtx<'_>) -> Option<toml::Table> {
    if !ctx.parse.errors.is_empty() {
        return None;
    }

    toml::from_str(ctx.file.contents).ok()
}

fn nested_table<'a>(document: &'a toml::Table, path: &[&str]) -> Option<&'a toml::Table> {
    let mut table = document;

    for segment in path {
        table = table.get(*segment)?.as_table()?;
    }

    Some(table)
}

fn inherits_workspace(value: &toml::Value) -> bool {
    value
        .as_table()
        .and_then(|table| table.get("workspace"))
        .and_then(toml::Value::as_bool)
        == Some(true)
}

fn rust_version_supports_edition(rust_version: &str, edition: i64) -> bool {
    let minimum_minor = match edition {
        ..=2015 => 0,
        2016..=2018 => 31,
        2019..=2021 => 56,
        _ => 85,
    };
    let mut components = rust_version.split('.');
    let major = components
        .next()
        .and_then(|value| value.parse::<u64>().ok());
    let minor = components
        .next()
        .and_then(|value| value.parse::<u64>().ok());

    major
        .zip(minor)
        .is_some_and(|(major, minor)| major > 1 || (major == 1 && minor >= minimum_minor))
}

fn header_name(line: &str) -> Option<&str> {
    line.trim()
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .map(str::trim)
}

fn section_line(lines: &[&str], section: &str) -> usize {
    lines
        .iter()
        .position(|line| header_name(line) == Some(section))
        .map_or(1, |index| index + 1)
}

fn key_line(lines: &[&str], section: &str, key: &str) -> usize {
    if let Some(start) = lines
        .iter()
        .position(|line| header_name(line) == Some(section))
    {
        for (index, line) in lines.iter().enumerate().skip(start + 1) {
            let trimmed = line.trim();

            if header_name(trimmed).is_some() {
                break;
            }

            let assigned = trimmed
                .split_once('=')
                .map(|(k, _)| k.trim().trim_matches('"'));

            if assigned == Some(key) {
                return index + 1;
            }
        }
    }

    let key_line = lines.iter().position(|line| {
        header_name(line).is_some_and(|header| {
            header
                .strip_prefix(section)
                .and_then(|rest| rest.strip_prefix('.'))
                == Some(key)
        })
    });

    key_line.map_or_else(|| section_line(lines, section), |index| index + 1)
}
