use googletest::prelude::*;

use super::*;
use crate::{file, temporary::Directory};

#[gtest]
fn invalid_rust_in_one_fix_aborts_the_whole_batch_before_writes() -> Result<()> {
    let directory = Directory::new().or_fail()?;
    let first = directory.path().join("first.rs");
    let second = directory.path().join("second.rs");
    let first_source = "fn first() {}\n";
    let second_source = "fn second() {}\n";

    file::write_text(&first, first_source).or_fail()?;
    file::write_text(&second, second_source).or_fail()?;
    let snapshots = SourceSnapshots::from([
        ("first.rs".to_string(), checksum::bytes(first_source)),
        ("second.rs".to_string(), checksum::bytes(second_source)),
    ]);
    let error = apply_fixes(
        &[
            (
                "first.rs".to_string(),
                Fix::replace_line(1, "fn first() { let value = 1; }"),
            ),
            (
                "second.rs".to_string(),
                Fix::replace_line(1, "fn second( {"),
            ),
        ],
        &snapshots,
        directory.path(),
    )
    .unwrap_err();

    verify_that!(error.to_string(), contains_substring("invalid Rust"))?;
    verify_eq!(file::read_text(&first).or_fail()?, first_source)?;

    verify_eq!(file::read_text(&second).or_fail()?, second_source)
}

#[gtest]
fn invalid_toml_fix_is_rejected_before_writing() -> Result<()> {
    let directory = Directory::new().or_fail()?;
    let path = directory.path().join("Cargo.toml");
    let source = "[package]\nname = \"fixture\"\n";

    file::write_text(&path, source).or_fail()?;
    let snapshots = SourceSnapshots::from([("Cargo.toml".to_string(), checksum::bytes(source))]);
    let error = apply_fixes(
        &[("Cargo.toml".to_string(), Fix::replace_line(1, "[package"))],
        &snapshots,
        directory.path(),
    )
    .unwrap_err();

    verify_that!(error.to_string(), contains_substring("invalid TOML"))?;

    verify_eq!(file::read_text(&path).or_fail()?, source)
}

#[gtest]
fn invalid_tree_fix_aborts_the_whole_batch_before_writes() -> Result<()> {
    let directory = Directory::new().or_fail()?;
    let first = directory.path().join("first.rs");
    let second = directory.path().join("second.rs");
    let first_source = "fn first() {}\n";
    let second_source = "fn second() {}\n";

    file::write_text(&first, first_source).or_fail()?;
    file::write_text(&second, second_source).or_fail()?;
    let snapshots = SourceSnapshots::from([
        ("first.rs".to_string(), checksum::bytes(first_source)),
        ("second.rs".to_string(), checksum::bytes(second_source)),
    ]);
    let error = apply_tree_fixes(
        &[
            TreeFix {
                rel: "first.rs".to_string(),
                rule: "fixture",
                replacement: "fn first() { let value = 1; }\n".to_string(),
            },
            TreeFix {
                rel: "second.rs".to_string(),
                rule: "fixture",
                replacement: "fn second( {\n".to_string(),
            },
        ],
        &snapshots,
        directory.path(),
    )
    .unwrap_err();

    verify_that!(error.to_string(), contains_substring("invalid Rust"))?;
    verify_eq!(file::read_text(&first).or_fail()?, first_source)?;

    verify_eq!(file::read_text(&second).or_fail()?, second_source)
}
