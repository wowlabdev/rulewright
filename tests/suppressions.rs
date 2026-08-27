mod common;

use std::fs;

use common::{initialize, root_package, run, text, write};

fn project(source: &str) -> tempfile::TempDir {
    let temporary = tempfile::tempdir().expect("temporary project should be created");

    root_package(temporary.path(), "suppression-fixture", source);
    initialize(temporary.path());

    temporary
}

#[test]
fn next_line_suppression_hides_only_the_named_rule() {
    let temporary = project(
        r#"pub fn boundary() {
    // #rw(rust_dbg) value is intentionally visible while probing this boundary
    dbg!("/home/dev/private-fixture");
}
"#,
    );

    let debug = run(temporary.path(), &["--rule", "rust_dbg"]);
    let home_path = run(temporary.path(), &["--rule", "rust_abs_home_path"]);

    assert!(debug.status.success(), "{}", text(&debug));
    assert!(!home_path.status.success());
    assert!(text(&home_path).contains("src/lib.rs:3"));
}

#[test]
fn file_block_function_and_wildcard_scopes_stop_at_their_boundaries() {
    let temporary = project(
        r#"// #rw(file: rust_panic) this fixture intentionally exercises panic suppression

// #rw(fn: rust_dbg) this whole function is an instrumentation fixture
pub fn function_scope() {
    dbg!(1);
}

pub fn block_scope() {
    // #rw(block: rust_dbg) this branch is an instrumentation fixture
    if true {
        dbg!(2);
    }
    dbg!(3);
}

pub fn wildcard_scope() {
    // #rw(*) generated fixture line is exempt from every selected rule
    dbg!(panic!("hidden"));
    dbg!(4);
}
"#,
    );

    let panic = run(temporary.path(), &["--rule", "rust_panic"]);
    let debug = run(temporary.path(), &["--rule", "rust_dbg"]);
    let debug_text = text(&debug);

    assert!(panic.status.success(), "{}", text(&panic));
    assert!(!debug.status.success());
    assert!(!debug_text.contains("src/lib.rs:5"), "{debug_text}");
    assert!(!debug_text.contains("src/lib.rs:11"), "{debug_text}");
    assert!(!debug_text.contains("src/lib.rs:18"), "{debug_text}");
    assert!(debug_text.contains("src/lib.rs:13"), "{debug_text}");
    assert!(debug_text.contains("src/lib.rs:19"), "{debug_text}");
}

#[test]
fn directive_validation_ignores_string_lookalikes_and_rejects_bad_comments() {
    let temporary = project(
        r##"pub const NORMAL: &str = "// #rw(unknown_rule) not a comment";
pub const RAW: &str = r#"// #rw(rust_dbg) not a comment"#;
"##,
    );

    let lookalikes = run(temporary.path(), &["--rule", "rust_rulewright_directives"]);

    assert!(lookalikes.status.success(), "{}", text(&lookalikes));

    write(
        &temporary.path().join("src/lib.rs"),
        "// #rw(unknown_rule) typo should be reported\npub fn clean() {}\n",
    );
    let unknown = run(temporary.path(), &["--rule", "rust_rulewright_directives"]);

    assert!(!unknown.status.success());
    assert!(text(&unknown).contains("unknown rule `unknown_rule`"));

    write(
        &temporary.path().join("src/lib.rs"),
        "// #rw(rust_dbg)\npub fn clean() {}\n",
    );
    let missing_reason = run(temporary.path(), &["--rule", "rust_rulewright_directives"]);

    assert!(!missing_reason.status.success());
    assert!(text(&missing_reason).contains("requires a reason"));
}

#[test]
fn report_and_clean_preserve_live_targets_and_reason_text() {
    let source = "// #rw(rust_dbg, rust_panic) debug output is intentional in this probe\npub fn probe() { dbg!(1); }\n";
    let temporary = project(source);
    let source_path = temporary.path().join("src/lib.rs");
    let report = run(temporary.path(), &["--suppressions"]);
    let report_text = text(&report);

    assert!(report.status.success(), "{report_text}");
    assert!(report_text.contains("rust_dbg (1 suppression(s))"));
    assert!(report_text.contains("rust_panic (1 suppression(s))"));
    assert!(report_text.contains("debug output is intentional in this probe"));

    let dry_run = run(temporary.path(), &["clean", "--dry-run"]);

    assert!(dry_run.status.success(), "{}", text(&dry_run));
    assert_eq!(
        fs::read_to_string(&source_path).expect("fixture source should be readable"),
        source
    );

    let clean = run(temporary.path(), &["clean"]);
    let cleaned = fs::read_to_string(source_path).expect("cleaned source should be readable");

    assert!(clean.status.success(), "{}", text(&clean));
    assert_eq!(
        cleaned,
        "// #rw(rust_dbg) debug output is intentional in this probe\npub fn probe() { dbg!(1); }\n"
    );
}

#[test]
fn clean_keeps_a_live_full_source_suppression_inside_a_test() {
    let source = r"// #rw(fn: rust_sorted) test fixture intentionally keeps reverse-order values
#[test]
fn reverse_order_fixture() {
    let values = [
        // #rw:sorted(asc)
        ZEBRA,
        ALPHA,
    ];
}
";
    let temporary = project(source);
    let analysis = run(temporary.path(), &["--rule", "rust_sorted"]);
    let clean = run(temporary.path(), &["--rule", "rust_sorted", "clean"]);
    let contents = fs::read_to_string(temporary.path().join("src/lib.rs"))
        .expect("fixture source should be readable");

    assert!(analysis.status.success(), "{}", text(&analysis));
    assert!(clean.status.success(), "{}", text(&clean));
    assert_eq!(contents, source);
}
