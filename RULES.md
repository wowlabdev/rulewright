# rulewright — Rust source code linter

Configured rules for the target Rust workspace. Runs beyond what clippy and rustfmt cover.

## Alignment guide

`// #rw:aligned` is not a suppression. It opts the immediately following table-like block into `rust_aligned`:

```rust
// #rw:aligned
Parser    => "parser";
Formatter => "formatter";
Writer    => "writer";
```

For blocks with at least two rows, Rulewright aligns `=>`, corresponding commas, and trailing `//` comments. The region ends at a blank line, a common closing delimiter, or another `#rw:` marker. Put the marker directly above the small block it describes; unmarked source is left alone.

Array and slice tables can also keep each simple tuple on one line:

```rust
#[rustfmt::skip]
let cases = [
    // #rw:aligned
    (SHORT,     "first"),
    (LONG_NAME, "second"),
];
```

`rulewright --fix` can add missing padding and collapse wrapped tuple rows when every field is a literal or path. Calls, blocks, nested tuples, and other expressions are reported without an automatic rewrite. Rustfmt removes manual column padding, so use `#[rustfmt::skip]` on the containing item when the layout must survive `cargo fmt`.

## Severity levels

| Severity | Meaning                                                              |
| -------- | -------------------------------------------------------------------- |
| high     | Likely bug, security issue, or correctness problem. Fix these first. |
| medium   | Potential bug, performance issue, or maintainability concern.        |
| low      | Style nit or minor improvement. Safe to suppress with good reason.   |

## Suppression directives

When a rule fires on code that is intentionally written that way, suppress it with a directive comment:

- `// #rw(rule) reason` — suppress the next line for this rule
- `// #rw(rule1, rule2) reason` — suppress the next line for multiple rules
- `// #rw(file: rule) reason` — skip the entire file for this rule
- `// #rw(block: rule) reason` — suppress until blank line or closing brace
- `// #rw(fn: rule) reason` — suppress until end of next function
- `// #rw(*) reason` — suppress all rules (next line)

Rule names and a reason after `)` are required. Missing either is a violation.

## Rules

| Rule                               | Severity | Type           | Enabled | Fixable | Description                                                                                                                                                                     |
| ---------------------------------- | -------- | -------------- | ------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| rust_abs_home_path                 | medium   | rust-line      | yes     | no      | Ban hardcoded home directory paths like `/Users/` or `/home/` in string literals.                                                                                               |
| rust_aligned                       | low      | rust-line      | yes     | yes     | Enforce column alignment in regions marked with `// #rw:aligned`.                                                                                                               |
| rust_alloc_in_loop                 | medium   | rust-ast       | no      | no      | Flag `format!()` and `.to_string()` inside loops.                                                                                                                               |
| rust_allow_reason                  | low      | rust-line      | yes     | no      | Require a `reason = "..."` or comment explaining why `#[allow(...)]`/`#[expect(...)]` is used.                                                                                  |
| rust_ambient_syscall               | medium   | rust-ast       | no      | no      | Flag ambient I/O, clock, env, and entropy calls in library code.                                                                                                                |
| rust_ambiguous_unicode             | high     | rust-line      | yes     | no      | Ban Unicode characters visually confusable with ASCII (homoglyphs).                                                                                                             |
| rust_asref_bound_on_type           | low      | rust-ast       | yes     | no      | Flag struct/enum generic parameters bounded by `AsRef<…>` and stored in fields.                                                                                                 |
| rust_assert_side_effects           | high     | rust-ast       | yes     | no      | Ban compound assignments (`+=`, `-=`) inside `debug_assert!` macros.                                                                                                            |
| rust_assoc_fn_no_self              | low      | rust-ast       | yes     | no      | Flag inherent associated fns that neither take nor return the impl type — make them free functions.                                                                             |
| rust_async_loop_no_yield           | low      | rust-ast       | yes     | no      | Flag loops in async contexts whose bodies never `.await` (CPU-bound work without yield points).                                                                                 |
| rust_attr_order                    | low      | rust-ast       | yes     | yes     | Require item attributes to be ordered as docs, derives, then other attributes.                                                                                                  |
| rust_banner_comments               | low      | rust-line      | yes     | yes     | Disallow decorative separator and framed banner comments.                                                                                                                       |
| rust_bidirectional_unicode         | high     | rust-line      | yes     | no      | Ban Unicode bidi control characters that enable trojan-source attacks.                                                                                                          |
| rust_bool_params                   | medium   | rust-ast       | yes     | no      | Flag functions with threshold+ `bool` parameters (error-prone API design).                                                                                                      |
| rust_box_leak                      | high     | rust-line      | yes     | no      | Require `SAFETY` or `LEAK` comment on `Box::leak()` calls.                                                                                                                      |
| rust_box_vec                       | medium   | rust-ast       | yes     | no      | Ban `Box<Vec<T>>`, `Box<String>`, `Box<Box<T>>` (unnecessary double indirection).                                                                                               |
| rust_build_rs_external_tool        | medium   | rust-ast       | yes     | no      | Flag build.rs usage of external tools, hard-required env vars, and build-time binding generation.                                                                               |
| rust_builder_conventions           | medium   | rust-ast       | yes     | no      | Enforce builder conventions: chainable by-value setters named `x()`, a final `build()`, and `X::builder()` instead of `XBuilder::new()`.                                        |
| rust_builder_fallible_setter       | medium   | rust-ast       | yes     | no      | Flag builder setters returning `Result` — setters accept infallibly, validation belongs in `build()`.                                                                           |
| rust_builder_param                 | low      | rust-ast       | yes     | no      | Flag parameters typed `*Builder`/`*Factory` — ask for `impl Fn() -> T` instead.                                                                                                 |
| rust_busy_wait                     | medium   | rust-ast       | yes     | no      | Flag spin loops polling `try_recv`/`try_lock`/atomics without sleeping, yielding, or blocking.                                                                                  |
| rust_catch_unwind                  | high     | rust-ast       | yes     | no      | Require `// PANIC-BOUNDARY:` comment on `catch_unwind` calls.                                                                                                                   |
| rust_cfg_not_test                  | medium   | rust-line      | yes     | no      | Flag `#[cfg(not(test))]` — use dependency injection or feature flags instead.                                                                                                   |
| rust_clone_in_loop                 | medium   | rust-ast       | no      | no      | Flag `.clone()` and `.to_owned()` calls on loop-invariant receivers inside loop bodies.                                                                                         |
| rust_closure_dense_method_chain    | medium   | rust-ast       | no      | no      | Flag method-call chains containing at least the configured number of inline closure arguments.                                                                                  |
| rust_closure_param_position        | low      | rust-ast       | yes     | no      | Flag closure parameters that are not last, and fns taking more than one closure.                                                                                                |
| rust_collection_new_in_loop        | medium   | rust-ast       | no      | no      | Flag collection constructors (`Vec::new()`, `vec![]`, `with_capacity`, ...) bound via `let` inside loops.                                                                       |
| rust_collection_trait_completeness | low      | rust-ast       | yes     | no      | Require collection trait counterparts: `iter()` needs `impl IntoIterator for &T`, `iter_mut()` needs `impl IntoIterator for &mut T`, and `FromIterator`/`Extend` come in pairs. |
| rust_comment_space                 | low      | rust-line      | yes     | yes     | Require a space after `//` in comments (`//bad` -> `// good`).                                                                                                                  |
| rust_commented_code                | low      | rust-line      | yes     | no      | Detect blocks of commented-out code (2+ consecutive lines).                                                                                                                     |
| rust_concrete_io_param             | low      | rust-ast       | no      | no      | Flag fn parameters typed as concrete I/O handles like `File` or `TcpStream`.                                                                                                    |
| rust_const_fn_candidate            | low      | rust-ast       | yes     | no      | Flag syntactically simple functions worth evaluating as `const fn` candidates.                                                                                                  |
| rust_const_needs_doc               | low      | rust-ast       | no      | no      | Require a doc or line comment on private consts and statics holding literal values.                                                                                             |
| rust_conversion_self_convention    | medium   | rust-ast       | no      | no      | Enforce C-CONV receivers: `as_`/`to_` methods borrow (`&self`), `into_` methods consume (`self`).                                                                               |
| rust_ctor_new                      | low      | rust-ast       | yes     | no      | Flag public structs with `Default` but no `pub fn new` — constructors are static inherent methods (C-CTOR).                                                                     |
| rust_ctor_param_count              | medium   | rust-ast       | no      | no      | Flag constructors with too many parameters or runs of identically-typed primitives — cascade construction through helper types.                                                 |
| rust_cyclomatic_complexity         | medium   | rust-ast       | no      | no      | Flag functions with cyclomatic complexity > threshold.                                                                                                                          |
| rust_dbg                           | medium   | rust-ast       | yes     | yes     | Ban `dbg!()` macro calls in production code.                                                                                                                                    |
| rust_deep_exit                     | high     | rust-ast       | yes     | no      | Ban `std::process::exit()` in library code.                                                                                                                                     |
| rust_deeply_nested_types           | low      | rust-ast       | yes     | no      | Flag type annotations with > 3 levels of generic nesting.                                                                                                                       |
| rust_default_hasher                | low      | rust-ast       | no      | no      | Flag std `HashMap`/`HashSet` types and constructors that use the default SipHash hasher.                                                                                        |
| rust_deny_warnings                 | medium   | rust-line      | yes     | yes     | Ban `#![deny(warnings)]` — breaks on compiler upgrades.                                                                                                                         |
| rust_derive_order                  | low      | rust-ast       | yes     | yes     | Require traits inside derive attributes to be sorted alphabetically.                                                                                                            |
| rust_dll_boundary_types            | high     | rust-ast       | yes     | no      | Flag `String`, `Vec`, `Box`, `dyn` objects, `TypeId`, and `Instant` in `extern "C"` signatures.                                                                                 |
| rust_doc_comment_period            | low      | rust-line      | yes     | no      | Require doc comments to end with proper punctuation.                                                                                                                            |
| rust_doc_errors_section            | medium   | rust-ast       | yes     | no      | Require a `# Errors` section on documented pub fns returning `Result`.                                                                                                          |
| rust_doc_inline_reexport           | low      | rust-line      | yes     | no      | Require `#[doc(inline)]` on local re-exports and forbid it on external ones.                                                                                                    |
| rust_doc_panics_section            | medium   | rust-ast       | no      | no      | Require a `# Panics` section on documented pub fns that can panic.                                                                                                              |
| rust_doc_param_table               | low      | rust-line      | yes     | no      | Ban `# Parameters`/`# Arguments`/`# Params` sections in doc comments.                                                                                                           |
| rust_drop_panic                    | high     | rust-ast       | yes     | no      | Ban `panic!`, `.unwrap()`, `.expect()` inside `impl Drop`.                                                                                                                      |
| rust_dup_expressions               | high     | rust-ast       | yes     | no      | Flag identical sub-expressions like `x == x`, `a - a`, `b && b`.                                                                                                                |
| rust_duplicate_strings             | low      | rust-workspace | yes     | no      | Find long string literals repeated across files; full-workspace runs are authoritative.                                                                                         |
| rust_duplicate_words               | low      | rust-line      | yes     | yes     | Flag repeated words in comments like `the the` or `is is`.                                                                                                                      |
| rust_dyn_wrapper_in_api            | low      | rust-ast       | yes     | no      | Flag `Rc<dyn …>`/`Arc<dyn …>`/`Box<dyn …>` in pub fn params, returns, and pub struct fields.                                                                                    |
| rust_error_missing_traits          | medium   | rust-ast       | yes     | no      | Require `Display` and `std::error::Error` on public `*Error` types.                                                                                                             |
| rust_error_type_unit               | medium   | rust-ast       | yes     | no      | Flag `Result<T, ()>` return types — use a real error type.                                                                                                                      |
| rust_excessive_float_precision     | low      | rust-ast       | yes     | no      | Flag float literals with more significant digits than the type can represent.                                                                                                   |
| rust_exotic_numeric_api            | low      | rust-ast       | yes     | no      | Flag `Saturating`/`Wrapping`/`NonZero*` in pub fn signatures.                                                                                                                   |
| rust_expect_message                | low      | rust-ast       | yes     | no      | Require `.expect()` to have a meaningful message, not generic ones.                                                                                                             |
| rust_expect_over_allow             | medium   | rust-line      | yes     | no      | Flag `#[allow(...)]` in hand-written code — use `#[expect(..., reason = "...")]` instead.                                                                                       |
| rust_fallible_in_iterator          | medium   | rust-ast       | yes     | no      | Flag `.unwrap()`/`.expect()` inside iterator adapter closures.                                                                                                                  |
| rust_ffi_crate_naming              | low      | rust-line      | yes     | no      | Require `-ffi` naming for crates exporting C symbols and `-sys` naming for crates linking foreign C items.                                                                      |
| rust_ffi_in_core                   | medium   | rust-ast       | yes     | no      | Flag `#[no_mangle] extern "C"` exports and `#[repr(C)]` raw-pointer structs in non-FFI crates.                                                                                  |
| rust_ffi_thin_glue                 | low      | rust-ast       | yes     | no      | Flag `extern "C"` functions in `*-ffi` crates whose body exceeds the line threshold.                                                                                            |
| rust_first_doc_sentence            | low      | rust-line      | no      | no      | Require the first doc sentence to end on the first line within a word budget.                                                                                                   |
| rust_floating_point_eq             | high     | rust-ast       | yes     | no      | Flag direct `==`/`!=` comparison on `f32`/`f64` values.                                                                                                                         |
| rust_foreign_reexports             | medium   | rust-ast       | yes     | no      | Flag `pub use` re-exports of items from foreign crates.                                                                                                                         |
| rust_from_instead_of_as            | low      | rust-ast       | yes     | yes     | Flag `as` casts on suffixed literals — use `From`/`Into` instead.                                                                                                               |
| rust_future_send_assert            | low      | rust-ast       | yes     | no      | Require a compile-time `Send` assertion for every explicit `impl Future` in the same file.                                                                                      |
| rust_getter_prefix                 | low      | rust-ast       | no      | no      | Flag methods named `get_something` — Rust getters are named after the field (C-GETTER).                                                                                         |
| rust_glob_reexport                 | medium   | rust-ast       | yes     | no      | Flag `pub use foo::*` glob re-exports outside platform-cfg'd HAL forwarding.                                                                                                    |
| rust_global_state                  | medium   | rust-ast       | yes     | no      | Flag `static` items with interior mutability and all `thread_local!` state.                                                                                                     |
| rust_hardcoded_url                 | medium   | rust-line      | yes     | no      | Flag hardcoded URLs in source code (should use config/env).                                                                                                                     |
| rust_impl_into_for_owned           | medium   | rust-ast       | yes     | no      | Flag `impl Into<T> for X` — implement `From<X> for T` instead (gives Into for free).                                                                                            |
| rust_impl_member_order             | medium   | rust-ast       | no      | yes     | Require inherent impl members to follow the canonical category and visibility order.                                                                                            |
| rust_infallible_from_weak          | medium   | rust-ast       | yes     | no      | Flag `impl From<weak>` next to fallible construction of the same type.                                                                                                          |
| rust_inherent_before_trait_impl    | low      | rust-ast       | yes     | no      | Require an inherent impl to precede trait impls for the same local type.                                                                                                        |
| rust_inline_test_module_size       | low      | rust-ast       | yes     | no      | Flag `#[cfg(test)] mod` blocks spanning more than threshold lines.                                                                                                              |
| rust_large_async_local             | medium   | rust-ast       | yes     | no      | Flag by-value `[T; N]` locals and parameters over threshold bytes inside async fns and blocks.                                                                                  |
| rust_large_enum_variant            | medium   | rust-ast       | yes     | no      | Flag enum variants that are much larger than others (should Box the large variant).                                                                                             |
| rust_large_fn_params               | medium   | rust-ast       | yes     | no      | Flag functions with > threshold parameters.                                                                                                                                     |
| rust_large_stack_array             | high     | rust-ast       | yes     | no      | Flag large fixed-size arrays on the stack (>threshold bytes). WASM has limited stack.                                                                                           |
| rust_log_in_loop                   | low      | rust-ast       | yes     | no      | Flag logging macro invocations inside loop bodies in library code.                                                                                                              |
| rust_log_named_events              | low      | rust-line      | yes     | no      | Flag `event!(...)` invocations without a `name:` argument before the level.                                                                                                     |
| rust_long_compound_name            | low      | rust-ast       | yes     | no      | Flag type definitions whose CamelCase name compounds more than threshold words.                                                                                                 |
| rust_loop_to_while                 | low      | rust-ast       | yes     | no      | Flag `loop { if cond { break; } ... }` — use `while` instead.                                                                                                                   |
| rust_lossy_cast                    | medium   | rust-ast       | yes     | no      | Flag `as` casts to types that lose precision (`f32`, `u8`, `u16`, `i8`, `i16`).                                                                                                 |
| rust_macro_hidden_items            | medium   | rust-ast       | yes     | no      | Flag fixed-name `pub` items emitted from quote! bodies.                                                                                                                         |
| rust_magic_numbers                 | low      | rust-ast       | no      | no      | Flag numeric literals outside the configured allowlist for review.                                                                                                              |
| rust_manual_async_fn               | low      | rust-ast       | yes     | no      | Flag non-async functions that return `impl Future` by wrapping the whole body in one `async` block.                                                                             |
| rust_manual_error_impl             | low      | rust-ast       | yes     | no      | Reject hand-written `Display` and `Error` implementations for `*Error` types.                                                                                                   |
| rust_map_err_pure_wrap             | low      | rust-ast       | yes     | no      | Flag `.map_err(...)` that only wraps the error in another type — implement `From` and let `?` convert.                                                                          |
| rust_match_layout                  | low      | rust-ast       | yes     | no      | Keep match arms and `matches!` patterns visually structured.                                                                                                                    |
| rust_max_fn_lines                  | medium   | rust-ast       | yes     | no      | Flag functions longer than threshold lines.                                                                                                                                     |
| rust_max_nesting                   | medium   | rust-ast       | yes     | no      | Flag nesting depth > threshold levels.                                                                                                                                          |
| rust_mem_forget                    | high     | rust-ast       | yes     | no      | Require `LEAK` or `SAFETY` comment on `std::mem::forget()` calls.                                                                                                               |
| rust_missing_assert_message        | low      | rust-ast       | yes     | no      | Require a message argument on `assert!`, `assert_eq!`, `assert_ne!`.                                                                                                            |
| rust_missing_capacity              | low      | rust-ast       | yes     | no      | Flag collections built with `new()`/`default()` then grown inside a loop over a sized source.                                                                                   |
| rust_missing_debug                 | low      | rust-ast       | yes     | yes     | Require `#[derive(Debug)]` on public structs and enums.                                                                                                                         |
| rust_missing_error_context         | medium   | rust-ast       | yes     | no      | Flag `.map_err(\|_\| ...)` that discards the original error.                                                                                                                    |
| rust_mod_order                     | low      | rust-ast       | yes     | yes     | Require contiguous module-declaration blocks to be alphabetically sorted.                                                                                                       |
| rust_module_docs                   | medium   | rust-line      | yes     | no      | Require `//!` module docs at the top of `lib.rs` and `mod.rs` files.                                                                                                            |
| rust_module_prefix_in_name         | low      | rust-ast       | no      | no      | Flag pub type definitions whose name repeats the module name as a prefix (`FooId` in `foo.rs`).                                                                                 |
| rust_multiple_inherent_impl        | low      | rust-ast       | yes     | no      | Flag multiple `impl Foo` blocks for the same type in one file.                                                                                                                  |
| rust_mutex_in_async                | high     | rust-ast       | yes     | no      | Flag `std::sync::Mutex` usage in async functions under an async-mutex-only policy.                                                                                              |
| rust_native_escape_hatches         | low      | rust-ast       | yes     | no      | Require `unsafe fn from_native`, `into_native`, and `to_native` on public raw-pointer wrapper structs.                                                                          |
| rust_nested_smart_pointers         | medium   | rust-ast       | yes     | no      | Flag directly nested heap pointers (`Arc<Box<T>>`, `Rc<Rc<T>>`, ...) plus `Arc<Vec<T>>`/`Arc<String>`.                                                                          |
| rust_newtype_pub_field             | medium   | rust-ast       | yes     | no      | Flag pub single-field structs exposing a pub primitive/`&str`/`String` field.                                                                                                   |
| rust_no_prelude                    | high     | rust-line      | yes     | no      | Ban `prelude` module declarations and `prelude.rs`/`prelude/mod.rs` files.                                                                                                      |
| rust_non_exhaustive_on_public      | medium   | rust-ast       | no      | no      | Flag public enums without `#[non_exhaustive]` — prevents breaking changes when adding variants.                                                                                 |
| rust_nonsend_across_await          | medium   | rust-ast       | yes     | no      | Flag `Rc`/`RefCell` bindings in async code when an `.await` occurs later in the same block.                                                                                     |
| rust_ok_or_eager                   | low      | rust-ast       | yes     | yes     | Flag `.ok_or()`/`.unwrap_or()` with eagerly evaluated arguments.                                                                                                                |
| rust_owned_ref_param               | medium   | rust-ast       | no      | no      | Flag fn parameters typed `&String`, `&PathBuf`, `&Vec<T>`, `&OsString`.                                                                                                         |
| rust_padding                       | low      | rust-ast       | yes     | yes     | Require configurable blank-line boundaries between functions and distinct statement groups.                                                                                     |
| rust_panic                         | high     | rust-ast       | yes     | no      | Ban `unimplemented!()`, `todo!()`, and message-less `panic!()` in library code.                                                                                                 |
| rust_panic_in_result_fn            | high     | rust-ast       | no      | no      | Ban `panic!`, `.unwrap()`, `.expect()` in functions returning `Result`.                                                                                                         |
| rust_panic_message                 | medium   | rust-ast       | yes     | no      | Require a message on `unreachable!` and `debug_assert!*`.                                                                                                                       |
| rust_param_clump                   | low      | rust-workspace | yes     | no      | Find maximal parameter groups repeated across functions; full-workspace runs are authoritative.                                                                                 |
| rust_param_order_consistency       | low      | rust-ast       | yes     | no      | Flag related fns whose shared parameters appear in a different order.                                                                                                           |
| rust_println                       | medium   | rust-ast       | no      | no      | Ban `println!`/`eprintln!`/`print!`/`eprint!` outside test code.                                                                                                                |
| rust_proc_macro_thin_shim          | low      | rust-ast       | yes     | no      | Require proc-macro entry points to be thin `impl_crate::name(arg.into()).into()` shims.                                                                                         |
| rust_pub_api_docs                  | low      | rust-ast       | no      | no      | Require doc comments on public items.                                                                                                                                           |
| rust_pub_api_foreign_types         | low      | rust-ast       | no      | no      | Flag foreign crate types leaked through `pub` fn signatures, fields, and type aliases.                                                                                          |
| rust_pub_api_generic_nesting       | low      | rust-ast       | yes     | no      | Flag pub fn signatures, pub struct fields, and pub type aliases nesting one local generic instantiation inside another (e.g. `Service<Backend<Store>>`).                        |
| rust_pub_api_smart_pointers        | medium   | rust-ast       | yes     | no      | Flag `Rc`/`Arc`/`Box`/`RefCell`/`Cell`/`Mutex`/`RwLock` as the outermost type of pub fn params, returns, and pub struct fields.                                                 |
| rust_pub_use_grouping              | low      | rust-ast       | yes     | yes     | Require public re-exports from the same origin to be adjacent.                                                                                                                  |
| rust_pub_use_position              | low      | rust-ast       | yes     | yes     | Require top-level public imports to follow plain imports in a separate block.                                                                                                   |
| rust_public_error_enum             | medium   | rust-ast       | no      | no      | Flag `pub enum` named `*Error`/`*ErrorKind` — expose a situation-specific error struct with a private kind enum instead.                                                        |
| rust_range_over_rangebounds        | low      | rust-ast       | yes     | no      | Flag `pub` fn parameters typed `Range<T>` — accept `impl RangeBounds<T>` instead.                                                                                               |
| rust_recursive_fn                  | high     | rust-ast       | yes     | no      | Flag direct self-recursion (stack overflow risk, especially in WASM).                                                                                                           |
| rust_redundant_field_names         | low      | rust-ast       | yes     | yes     | Flag `Foo { x: x }` — use shorthand `Foo { x }` instead.                                                                                                                        |
| rust_rulewright_directives         | low      | rust-line      | yes     | no      | Enforce file-wide #rw directives at top of file with a blank line separator.                                                                                                    |
| rust_sensitive_debug               | high     | rust-ast       | yes     | no      | Flag `#[derive(Debug)]` on structs with sensitive fields like `password`.                                                                                                       |
| rust_similar_fns                   | low      | rust-workspace | yes     | no      | Find exact and near duplicate function bodies; full-workspace runs are authoritative.                                                                                           |
| rust_similar_structs               | low      | rust-workspace | yes     | no      | Find exact, near, and containment duplicate named-field structs; full-workspace runs are authoritative.                                                                         |
| rust_single_item_path              | medium   | rust-ast       | yes     | no      | Flag `pub use` re-exports that duplicate paths already public through a sibling `pub mod`.                                                                                      |
| rust_sorted                        | low      | rust-line      | yes     | yes     | Enforce ordering in contiguous regions marked with `#rw:sorted(asc)` or `#rw:sorted(desc)`.                                                                                     |
| rust_static_mut                    | high     | rust-line      | yes     | no      | Ban `static mut` declarations — use `AtomicT`, `Mutex`, or `OnceLock`.                                                                                                          |
| rust_string_error                  | medium   | rust-ast       | no      | no      | Reject `String` and `&str` as function error types.                                                                                                                             |
| rust_style                         | low      | rust-line      | yes     | yes     | Enforce no trailing whitespace, no tabs, no CRLF line endings.                                                                                                                  |
| rust_subtractive_feature_cfg       | medium   | rust-ast       | yes     | no      | Flag `#[cfg(not(feature = "..."))]` on `pub` items — features must be additive.                                                                                                 |
| rust_tautological_assert           | low      | rust-ast       | yes     | no      | Flag test asserts comparing a constant against a literal (or literal vs literal).                                                                                               |
| rust_thiserror_qualified           | low      | rust-ast       | yes     | yes     | Require thiserror derives to use the qualified `thiserror::Error` path.                                                                                                         |
| rust_todo                          | low      | rust-line      | yes     | no      | Require TODO/FIXME/HACK/XXX to have parenthesized context.                                                                                                                      |
| rust_too_many_lines_in_file        | medium   | rust-line      | yes     | no      | Flag files exceeding threshold lines.                                                                                                                                           |
| rust_trait_logic_not_inherent      | low      | rust-ast       | yes     | no      | Flag substantial logic in impls of locally-defined traits when the type has no same-named inherent method.                                                                      |
| rust_transmute_in_safe_fn          | high     | rust-ast       | yes     | no      | Flag `transmute` inside a safe `pub` fn.                                                                                                                                        |
| rust_transmute_usage               | high     | rust-ast       | yes     | no      | Require `SAFETY` comment on `std::mem::transmute` calls.                                                                                                                        |
| rust_type_def_ordering             | low      | rust-ast       | yes     | no      | Flag `impl` blocks that appear before their type definition.                                                                                                                    |
| rust_unbalanced_crate_root         | low      | rust-ast       | yes     | no      | Flag `lib.rs` roots that are flat item dumps (too many pub items) or empty shells (no pub items over many pub modules).                                                         |
| rust_unchecked_indexing            | low      | rust-ast       | yes     | no      | Flag `container[expr]` indexing with non-literal indices.                                                                                                                       |
| rust_unnecessary_collect           | low      | rust-ast       | yes     | no      | Flag `.collect().iter()` — remove the intermediate collection.                                                                                                                  |
| rust_unsafe_comment                | high     | rust-ast       | yes     | no      | Require `// SAFETY:` comment on `unsafe` blocks.                                                                                                                                |
| rust_unsafe_fn_safety_doc          | high     | rust-ast       | yes     | no      | Require a `# Safety` doc section or `// SAFETY:` comment on every `unsafe fn`.                                                                                                  |
| rust_unsafe_impl_send              | high     | rust-line      | yes     | no      | Flag `unsafe impl Send`/`Sync` without a `// SAFETY:` comment, and any generic (blanket) form.                                                                                  |
| rust_unsafe_without_ub_surface     | low      | rust-ast       | yes     | no      | Flag `unsafe fn` with no raw-pointer surface and no unsafe operations in the body.                                                                                              |
| rust_unwrap_in_lib                 | medium   | rust-ast       | no      | no      | Ban `.unwrap()` in library code.                                                                                                                                                |
| rust_vec_init_then_push            | low      | rust-ast       | yes     | no      | Flag `Vec::new()` immediately followed by `.push()` calls (use `vec![]` or `with_capacity`).                                                                                    |
| rust_vec_string_field              | low      | rust-ast       | no      | no      | Flag non-pub struct fields typed `Vec<String>` or `Vec<Vec<T>>`.                                                                                                                |
| rust_weasel_words                  | medium   | rust-ast       | yes     | no      | Flag type definitions whose name contains a weasel word like `Manager`, `Service`, or `Factory`.                                                                                |
| rust_where_clauses                 | low      | rust-ast       | yes     | no      | Require type-parameter trait bounds to use where clauses.                                                                                                                       |
| rust_wildcard_imports              | medium   | rust-ast       | yes     | no      | Ban `use foo::*` outside tests and preludes.                                                                                                                                    |
| rust_yoda_conditions               | low      | rust-ast       | yes     | no      | Flag reversed comparisons like `0 == x` — prefer `x == 0`.                                                                                                                      |
| toml_ambiguous_unicode             | high     | toml           | yes     | no      | Ban Unicode characters in TOML that are visually confusable with ASCII.                                                                                                         |
| toml_bidirectional_unicode         | high     | toml           | yes     | no      | Ban Unicode bidi control characters in TOML.                                                                                                                                    |
| toml_cargo_edition                 | medium   | toml           | yes     | no      | Require the workspace and non-inheriting members to target at least the configured Rust edition, with the matching virtual-workspace resolver.                                  |
| toml_cargo_feature_names           | low      | toml           | yes     | no      | Flag Cargo feature names with use-/with- prefixes or -support suffixes.                                                                                                         |
| toml_cargo_feature_no_std          | medium   | toml           | yes     | no      | Ban subtractive no-std Cargo features; provide an additive std feature instead.                                                                                                 |
| toml_cargo_msrv                    | low      | toml           | yes     | no      | Require the workspace to declare rust-version and members to inherit it instead of overriding it.                                                                               |
| toml_cargo_unused_deps             | medium   | workspace      | yes     | no      | Flag workspace-member dependencies that are never referenced by any Rust compile target.                                                                                        |
| toml_cargo_workspace_dep_features  | low      | toml           | yes     | no      | Flag [workspace.dependencies] entries that enable features outside the allowlist.                                                                                               |
| toml_cargo_workspace_lints         | medium   | toml           | yes     | no      | Require the workspace to enable the standard rust/clippy lint set and members to inherit it via `[lints] workspace = true`.                                                     |
| toml_validity                      | high     | toml           | yes     | no      | Reject TOML syntax errors and semantic conflicts such as duplicate keys.                                                                                                        |

## Rule details

### rust_abs_home_path

Ban hardcoded home directory paths like `/Users/` or `/home/` in string literals.

> Absolute home paths break on other machines and in CI. Use environment variables or relative paths.

|          |           |
| -------- | --------- |
| Severity | medium    |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | no        |

**Bad (triggers violation):**

_Users path in string_

```rust
let p = "/Users/john/file";
```

_home path in string_

```rust
let p = "/home/user/data";
```

_Windows path in string_

```rust
let p = "C:\\Users\\john\\file";
```

**Good (passes):**

_tmp path_

```rust
let p = "/tmp/file";
```

_comment with path_

```rust
// /Users/foo
```

_no quotes_

```rust
let users = get_users();
```

### rust_aligned

Enforce column alignment in regions marked with `// #rw:aligned`.

> Consistent column alignment in marked regions makes tabular data and match arms easier to scan.

|          |           |
| -------- | --------- |
| Severity | low       |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | yes       |

**Bad (triggers violation):**

_arrow misaligned_

```rust
// #rw:aligned
Parser    => "a";
Writer => "b";
Files     => "c";
```

_comma misaligned_

```rust
// #rw:aligned
call(a,  "x",  TypeA);
call(long_name, "y", TypeB);
```

_trailing comments misaligned_

```rust
// #rw:aligned
(A, 1),      // first
(B, 2), // second
```

_wrapped tuple row_

```rust
// #rw:aligned
(
    SHORT,
    "first",
),
(LONG_NAME, "second"),
```

**Good (passes):**

_arrow aligned_

```rust
// #rw:aligned
Parser    => "a";
Writer    => "b";
Files     => "c";
```

_comma aligned_

```rust
// #rw:aligned
call(a,  "x",  TypeA);
call(b,  "y",  TypeB);
```

_no marker_

```rust
Parser    => "a";
Writer => "b";
```

_trailing comments aligned_

```rust
// #rw:aligned
(A, 1), // first
(B, 2), // second
```

### rust_alloc_in_loop

Flag `format!()` and `.to_string()` inside loops.

> These syntax forms create a new String each iteration. In a measured hot loop, reuse a buffer, write into existing storage, or move the conversion outside the loop when possible. Intentional allocations and cold loops may be excluded through configuration or a documented suppression.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | no       |
| Fixable  | no       |

**Bad (triggers violation):**

_format! in for loop_

```rust
fn f() { for i in 0..10 { let _ = format!("item {}", i); } }
```

_to_string in while loop_

```rust
fn f() { let mut i = 0; while i < 10 { let _ = i.to_string(); i += 1; } }
```

**Good (passes):**

_push_str does not necessarily allocate_

```rust
fn f() { let mut s = String::new(); for _ in 0..10 { s.push_str("x"); } }
```

_format_args borrows its arguments_

```rust
fn f() { for i in 0..10 { let _ = format_args!("item {}", i); } }
```

_format! outside loop_

```rust
fn f() { let _ = format!("hello"); }
```

_to_string outside loop_

```rust
fn f() { let _ = 42.to_string(); }
```

_push_str outside loop_

```rust
fn f() { let mut s = String::new(); s.push_str("x"); }
```

_format! in loop in test_

```rust
#[cfg(test)]
mod tests {
    fn t() { for i in 0..10 { let _ = format!("x{}", i); } }
}
```

### rust_allow_reason

Require a `reason = "..."` or comment explaining why `#[allow(...)]`/`#[expect(...)]` is used.

> Unexplained lint overrides hide the intent behind suppressing a warning, making it unclear if the suppression is still needed.

|          |           |
| -------- | --------- |
| Severity | low       |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | no        |

**Bad (triggers violation):**

_allow without comment_

```rust
#[allow(dead_code)]
```

_expect without reason_

```rust
#[expect(clippy::unused_async)]
```

_multiline expect without reason_

```rust
#[expect(
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
)]
```

_module-level allow without comment_

```rust
#![allow(clippy::too_many_arguments)]
```

**Good (passes):**

_allow with inline comment_

```rust
#[allow(dead_code)] // webhook response fields
```

_allow with preceding comment_

```rust
// DBC fields use PascalCase
#[allow(non_snake_case)]
```

_allow with reason argument_

```rust
#[allow(dead_code, reason = "webhook response fields")]
```

_expect with reason argument_

```rust
#[expect(clippy::unused_async, reason = "API fixed, will use I/O later")]
```

_multiline allow with reason argument_

```rust
#[allow(
    dead_code,
    reason = "webhook response fields",
)]
```

_multiline expect with reason argument_

```rust
#[expect(
    clippy::cast_precision_loss,
    reason = "sample count enters the f64 domain",
)]
```

_multiline module expect with reason argument_

```rust
#![expect(
    clippy::doc_markdown,
    reason = "generated documentation is external",
)]
```

_module-level allow with comment_

```rust
// compatibility callback has many parameters by design
#![allow(clippy::too_many_arguments)]
```

_non-allow attr_

```rust
#[derive(Debug)]
```

_deny attr_

```rust
#[deny(unused)]
```

### rust_ambient_syscall

Flag ambient I/O, clock, env, and entropy calls in library code.

> Syscalls called ambiently cannot be mocked, making edge cases untestable — inject them through an abstraction.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | no       |
| Fixable  | no       |

**Bad (triggers violation):**

_ambient fs read_

```rust
fn load() { let _ = std::fs::read("cfg.toml"); }
```

_ambient fs write via import_

```rust
use std::fs;
fn save() { let _ = fs::write("out.bin", b"data"); }
```

_ambient File open_

```rust
fn open() { let _ = std::fs::File::open("cfg.toml"); }
```

_ambient clock_

```rust
fn stamp() { let _ = std::time::Instant::now(); }
```

_ambient system time_

```rust
use std::time::SystemTime;
fn stamp() { let _ = SystemTime::now(); }
```

_ambient env read_

```rust
fn cfg() { let _ = std::env::var("MODE"); }
```

_ambient network connect_

```rust
fn dial(addr: &str) { let _ = std::net::TcpStream::connect(addr); }
```

_ambient entropy_

```rust
fn roll() -> u32 { rand::random() }
```

_ambient thread_rng import_

```rust
use rand::thread_rng;
fn roll() { let _ = thread_rng(); }
```

**Good (passes):**

_injected clock is fine_

```rust
fn stamp(clock: &dyn Clock) { let _ = clock.now(); }
```

_unrelated fs module_

```rust
fn load() { let _ = custom::fs::read(1); }
```

_syscall in test module_

```rust
#[cfg(test)]
mod tests {
    fn t() { let _ = std::time::Instant::now(); }
}
```

### rust_ambiguous_unicode

Ban Unicode characters visually confusable with ASCII (homoglyphs).

> Homoglyphs like Cyrillic U+0430 vs Latin 'a' make code read one way but compile another (trojan-source risk).

|          |           |
| -------- | --------- |
| Severity | high      |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | no        |

**Bad (triggers violation):**

_cyrillic a homoglyph_

```rust
let x = "pаssword";
```

_en dash instead of minus_

```rust
// range 1–5
```

_curly apostrophe_

```rust
// don’t
```

_multiplication sign_

```rust
// 4×4 matrix
```

**Good (passes):**

_normal ASCII_

```rust
let x = "hello world";
```

_unambiguous non-ASCII_

```rust
// résumé
```

### rust_asref_bound_on_type

Flag struct/enum generic parameters bounded by `AsRef<…>` and stored in fields.

> AsRef bounds on data-carrying type parameters infect every use of the type; own concrete data like `String` instead.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_inline AsRef bound stored in field_

```rust
struct User<T: AsRef<str>> { name: T }
```

_where-clause AsRef bound stored in field_

```rust
struct User<T> where T: AsRef<str> { name: T }
```

_bounded parameter nested in field type_

```rust
struct User<T: AsRef<str>> { names: Vec<T> }
```

_enum variant stores AsRef-bounded parameter_

```rust
enum E<T: AsRef<str>> { A(T) }
```

_qualified AsRef bound_

```rust
struct User<T: std::convert::AsRef<str>> { name: T }
```

**Good (passes):**

_concrete owned field_

```rust
struct User { name: String }
```

_non-AsRef bound is fine_

```rust
struct User<T: Clone> { name: T }
```

_AsRef bound on fn generic is fine_

```rust
fn print<T: AsRef<str>>(x: T) {}
```

_bounded parameter not stored in fields_

```rust
struct S<T: AsRef<str>> { count: usize }
```

_AsRef-bounded struct in test module_

```rust
#[cfg(test)]
mod tests {
    struct User<T: AsRef<str>> { name: T }
}
```

### rust_assert_side_effects

Ban compound assignments (`+=`, `-=`) inside `debug_assert!` macros.

> Compound assignments inside debug_assert! are side effects that vanish in release builds, causing silent behavior changes.

|          |          |
| -------- | -------- |
| Severity | high     |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_compound assign in debug_assert_

```rust
fn f() { let mut x = 0; debug_assert!({ x += 1; x > 0 }); }
```

_subtract assign in debug_assert_

```rust
fn f() { let mut x = 5; debug_assert!({ x -= 1; x > 0 }); }
```

**Good (passes):**

_simple comparison_

```rust
fn f() { debug_assert!(x > 0); }
```

_debug_assert_eq passes_

```rust
fn f() { debug_assert_eq!(a, b); }
```

_compound assign in test module_

```rust
#[cfg(test)]
mod tests {
    fn t() { let mut x = 0; debug_assert!({ x += 1; x > 0 }); }
}
```

### rust_assoc_fn_no_self

Flag inherent associated fns that neither take nor return the impl type — make them free functions.

> Regular functions are first-class in Rust; computation unrelated to a receiver hosted in an impl block adds Type:: noise for no benefit.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_unrelated helper in impl_

```rust
struct Db;
impl Db {
    fn check_parameters(input: &str) -> bool {
        input.is_empty()
    }
}
```

**Good (passes):**

_constructor_

```rust
struct Db;
impl Db {
    fn new() -> Self {
        Db
    }
}
```

_returns the impl type_

```rust
struct Db;
impl Db {
    fn connect(url: &str) -> Db {
        Db
    }
}
```

_takes Self parameters_

```rust
struct Db;
impl Db {
    fn merge(a: Self, b: Self) -> u32 {
        0
    }
}
```

_method with receiver_

```rust
struct Db;
impl Db {
    fn query(&self) {}
}
```

_from\_ prefixed_

```rust
struct Db;
impl Db {
    fn from_code(code: u32) -> u32 {
        code
    }
}
```

_parameter type mentioning impl type_

```rust
struct Config;
impl Config {
    fn validate_rule(rule: &RuleConfig) -> bool {
        true
    }
}
```

_trait impl_

```rust
struct S;
trait Calc {
    fn calc(x: u32) -> u32;
}
impl Calc for S {
    fn calc(x: u32) -> u32 {
        x
    }
}
```

_unrelated helper in test module_

```rust
#[cfg(test)]
mod tests {
    struct Db;
    impl Db {
        fn check_parameters(input: &str) -> bool {
            input.is_empty()
        }
    }
}
```

### rust_async_loop_no_yield

Flag loops in async contexts whose bodies never `.await` (CPU-bound work without yield points).

> CPU-bound async loops must cooperatively yield (yield_now().await) so they do not starve the runtime.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_async for loop with call but no await_

```rust
async fn f(xs: &[u32]) { for x in xs { process(*x); } }
```

_loop in async block without await_

```rust
fn g() { let _fut = async { loop { step(); } }; }
```

_three statements without calls or await_

```rust
async fn f(xs: &[u32]) { for x in xs { let a = *x; let b = a + 1; let _c = b - a; } }
```

**Good (passes):**

_loop yields via yield_now().await_

```rust
async fn f(xs: &[u32]) { for x in xs { process(*x); tokio::task::yield_now().await; } }
```

_sync fn loop is fine_

```rust
fn f(xs: &[u32]) { for x in xs { process(*x); } }
```

_tiny accumulation loop without calls_

```rust
async fn f(xs: &[u32]) -> u32 { let mut total = 0; for x in xs { total += x; } total }
```

_loop inside sync closure in async fn_

```rust
async fn f(xs: &[u32]) { let sum = || { for x in xs { process(*x); } }; sum(); }
```

_await nested deeper in loop body_

```rust
async fn f(n: u32) { for _ in 0..n { if n > 1 { step().await; } } }
```

_async loop in test module_

```rust
#[cfg(test)]
mod tests {
    async fn t(xs: &[u32]) { for x in xs { process(*x); } }
}
```

### rust_attr_order

Require item attributes to be ordered as docs, derives, then other attributes.

> Consistent attribute categories keep API documentation and generated traits prominent.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | yes      |

**Bad (triggers violation):**

_derive follows other attribute_

```rust
#[cfg(test)]
#[derive(Debug)]
struct Item;
```

_docs must be first_

```rust
#[derive(Debug)]
/// Item docs.
struct Item;
```

**Good (passes):**

_docs derive then other attributes_

```rust
/// Item docs.
#[derive(Debug)]
#[cfg(test)]
struct Item;
```

_stable order within category_

```rust
#[cfg(unix)]
#[allow(dead_code)]
struct Item;
```

### rust_banner_comments

Disallow decorative separator and framed banner comments.

> Structural code organization communicates sections more clearly than punctuation banners.

|          |           |
| -------- | --------- |
| Severity | low       |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | yes       |

**Bad (triggers violation):**

_plain separator_

```rust
// ----------------
```

_framed banner_

```rust
// ==== parsing ====
```

_mixed decorative separator_

```rust
// __~~__
```

**Good (passes):**

_ordinary comment_

```rust
// Parsing helpers live below.
```

_short punctuation_

```rust
// --- note ---
```

_comment-like string_

```rust
let banner = "// --------";
```

### rust_bidirectional_unicode

Ban Unicode bidi control characters that enable trojan-source attacks.

> Bidi control characters can reorder displayed code to hide malicious logic (CVE-2021-42574).

|          |           |
| -------- | --------- |
| Severity | high      |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | no        |

**Bad (triggers violation):**

_bidi LRE character_

```rust
let x = "‪test";
```

_bidi RLO character_

```rust
let x = "‮test";
```

_bidi LRM character_

```rust
let x = "‎test";
```

**Good (passes):**

_normal ASCII_

```rust
let x = "hello world";
```

### rust_bool_params

Flag functions with threshold+ `bool` parameters (error-prone API design).

> Multiple bool parameters are easy to mix up at call sites. Use an enum to make each argument self-documenting.

|                  |                  |
| ---------------- | ---------------- |
| Severity         | medium           |
| Type             | rust-ast         |
| Enabled          | yes              |
| Fixable          | no               |
| Param: threshold | i64, default = 2 |

**Bad (triggers violation):**

_two bool params_

```rust
fn f(a: bool, b: bool) {}
```

_three bool params_

```rust
fn f(a: bool, b: bool, c: bool) {}
```

**Good (passes):**

_one bool param_

```rust
fn f(a: bool, b: i32) {}
```

_no bool params_

```rust
fn f(a: i32, b: i32) {}
```

_self not counted_

```rust
struct S;
impl S {
  fn f(&self, a: bool) {}
}
```

_bool params in test module_

```rust
#[cfg(test)]
mod tests {
  fn f(a: bool, b: bool) {}
}
```

### rust_box_leak

Require `SAFETY` or `LEAK` comment on `Box::leak()` calls.

> Box::leak intentionally creates a memory leak. A justification comment proves it was deliberate, not accidental.

|          |           |
| -------- | --------- |
| Severity | high      |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | no        |

**Bad (triggers violation):**

_Box::leak without comment_

```rust
let x = Box::leak(Box::new(42));
```

**Good (passes):**

_Box::leak with SAFETY comment_

```rust
// SAFETY: static lifetime needed for FFI
let x = Box::leak(Box::new(42));
```

_Box::leak with LEAK comment_

```rust
// LEAK: intentional for process lifetime
let x = Box::leak(Box::new(42));
```

_comment line not flagged_

```rust
// Box::leak example
```

_normal code_

```rust
let x = Box::new(42);
```

### rust_box_vec

Ban `Box<Vec<T>>`, `Box<String>`, `Box<Box<T>>` (unnecessary double indirection).

> Box<Vec<T>> adds a pointless heap indirection since Vec already heap-allocates. Use Vec<T> or Box<[T]>.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_Box<Vec<T>>_

```rust
fn f() { let x: Box<Vec<i32>> = Box::new(vec![]); }
```

_Box<String>_

```rust
fn f() { let x: Box<String> = Box::new(String::new()); }
```

_Box<Box<T>>_

```rust
fn f() { let x: Box<Box<i32>> = Box::new(Box::new(0)); }
```

**Good (passes):**

_Box<dyn Trait>_

```rust
fn f() { let x: Box<dyn Trait> = Box::new(foo); }
```

_comment and literal with patterns_

```rust
// Box<Vec<i32>> is bad
const NOTE: &str = "Box<String> and Box<Box<T>>";
```

### rust_build_rs_external_tool

Flag build.rs usage of external tools, hard-required env vars, and build-time binding generation.

> Builds must work out of the box with cargo and rustc alone — external tools and required env vars break every downstream consumer.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_external tool invocation_

```rust
fn main() { let _ = std::process::Command::new("cmake").status(); }
```

_required env var via unwrap_

```rust
fn main() { let _ = std::env::var("FOO_LIB_DIR").unwrap(); }
```

_required env var via expect_

```rust
fn main() { let _ = std::env::var("FOO_LIB_DIR").expect("FOO_LIB_DIR must be set"); }
```

_required env var via question mark_

```rust
fn main() -> Result<(), std::env::VarError> {
    let _ = std::env::var("FOO_LIB_DIR")?;
    Ok(())
}
```

_build-time bindgen_

```rust
fn main() { let _ = bindgen::Builder::default(); }
```

_build-time pkg_config probe_

```rust
fn main() { let _ = pkg_config::probe_library("foo"); }
```

**Good (passes):**

_compiler driver is allowed_

```rust
fn main() { let _ = std::process::Command::new("cc").status(); }
```

_defaulted env var is fine_

```rust
fn main() { let _ = std::env::var("PROFILE").unwrap_or_default(); }
```

_optional env var is fine_

```rust
fn main() { if let Ok(dir) = std::env::var("FOO_DIR") { let _ = dir; } }
```

_plain rerun directive_

```rust
fn main() { println!("cargo:rerun-if-changed=src/schema.json"); }
```

### rust_builder_conventions

Enforce builder conventions: chainable by-value setters named `x()`, a final `build()`, and `X::builder()` instead of `XBuilder::new()`.

> Builders that deviate from the canonical pattern break fluent construction and surprise users who expect X::builder()...build().

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_builder without build method_

```rust
pub struct ConnBuilder { retries: u32 }
impl ConnBuilder {
    pub fn retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }
}
```

_public new on builder_

```rust
pub struct ConnBuilder;
impl ConnBuilder {
    pub fn new() -> Self {
        ConnBuilder
    }
    pub fn build(self) -> u32 {
        0
    }
}
```

_set\_ prefixed setter_

```rust
pub struct ConnBuilder { port: u16 }
impl ConnBuilder {
    pub fn set_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }
    pub fn build(self) -> u16 {
        self.port
    }
}
```

_with\_ prefixed setter_

```rust
pub struct ConnBuilder { port: u16 }
impl ConnBuilder {
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }
    pub fn build(self) -> u16 {
        self.port
    }
}
```

_borrowing setter_

```rust
pub struct ConnBuilder { port: u16 }
impl ConnBuilder {
    pub fn port(&mut self, port: u16) -> &mut Self {
        self.port = port;
        self
    }
    pub fn build(self) -> u16 {
        self.port
    }
}
```

_buildable type without builder shortcut_

```rust
pub struct Conn;
pub struct ConnBuilder;
impl ConnBuilder {
    pub fn build(self) -> Conn {
        Conn
    }
}
```

**Good (passes):**

_conventional builder_

```rust
pub struct Foo;
pub struct FooBuilder { size: u32 }
impl Foo {
    pub fn builder() -> FooBuilder {
        FooBuilder { size: 0 }
    }
}
impl FooBuilder {
    pub fn size(mut self, size: u32) -> Self {
        self.size = size;
        self
    }
    pub fn build(self) -> Foo {
        Foo
    }
}
```

_builder without same-file impl_

```rust
pub struct ConnBuilder;
```

_private builder_

```rust
struct ConnBuilder;
impl ConnBuilder {
    fn new() -> Self {
        ConnBuilder
    }
}
```

_builder in test module_

```rust
#[cfg(test)]
mod tests {
    pub struct ConnBuilder;
    impl ConnBuilder {
        pub fn new() -> Self {
            ConnBuilder
        }
    }
}
```

### rust_builder_fallible_setter

Flag builder setters returning `Result` — setters accept infallibly, validation belongs in `build()`.

> Fallible setters force repeated error checks that add noise and still cannot guard interdependent conditions; a Result-carrying build() consolidates validation.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_fallible setter_

```rust
struct HostBuilder;
impl HostBuilder {
    fn port(self, port: u16) -> Result<Self, String> {
        Ok(self)
    }
    fn build(self) -> u32 {
        0
    }
}
```

**Good (passes):**

_fallible build_

```rust
struct HostBuilder;
impl HostBuilder {
    fn port(self, port: u16) -> Self {
        self
    }
    fn build(self) -> Result<u32, String> {
        Ok(0)
    }
}
```

_fallible try_build and finish_

```rust
struct HostBuilder;
impl HostBuilder {
    fn try_build(self) -> Result<u32, String> {
        Ok(0)
    }
    fn finish(self) -> Result<u32, String> {
        Ok(0)
    }
}
```

_fallible associated fn_

```rust
struct HostBuilder;
impl HostBuilder {
    fn parse(input: &str) -> Result<Self, String> {
        Ok(HostBuilder)
    }
}
```

_fallible method on non-builder_

```rust
struct Config;
impl Config {
    fn set(self, key: u32) -> Result<Self, String> {
        Ok(self)
    }
}
```

_fallible setter in test module_

```rust
#[cfg(test)]
mod tests {
    struct HostBuilder;
    impl HostBuilder {
        fn port(self, port: u16) -> Result<Self, String> {
            Ok(self)
        }
    }
}
```

### rust_builder_param

Flag parameters typed `*Builder`/`*Factory` — ask for `impl Fn() -> T` instead.

> Accepting factories or builders as parameters imports OO indirection; an impl Fn() -> T expresses repeatable instantiation idiomatically.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_builder parameter_

```rust
fn make(b: WidgetBuilder) {}
```

_factory reference parameter_

```rust
fn make(f: &WidgetFactory) {}
```

_builder parameter on method_

```rust
struct App;
impl App {
    fn install(&self, b: WidgetBuilder) {}
}
```

**Good (passes):**

_closure factory parameter_

```rust
fn make(f: impl Fn() -> Widget) {}
```

_plain parameter_

```rust
fn make(w: Widget) {}
```

_builder parameter in test module_

```rust
#[cfg(test)]
mod tests {
    fn make(b: WidgetBuilder) {}
}
```

### rust_busy_wait

Flag spin loops polling `try_recv`/`try_lock`/atomics without sleeping, yielding, or blocking.

> Hot spinning burns CPU cycles when no work is present. Sleep, yield_now, park, or block on recv() between polls.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_try_recv spin loop without sleep_

```rust
fn f(rx: std::sync::mpsc::Receiver<u8>) { loop { if let Ok(v) = rx.try_recv() { drop(v); } } }
```

_while spinning on atomic load condition_

```rust
fn f(flag: &std::sync::atomic::AtomicBool) { while flag.load(std::sync::atomic::Ordering::Acquire) {} }
```

_compare_exchange spin lock_

```rust
fn f(lock: &std::sync::atomic::AtomicBool) { loop { match lock.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed) { Ok(_) => break, Err(_) => {} } } }
```

_while spinning on try_lock condition_

```rust
fn f(m: &std::sync::Mutex<u8>) { while m.try_lock().is_err() {} }
```

**Good (passes):**

_try_recv loop with sleep_

```rust
fn f(rx: std::sync::mpsc::Receiver<u8>) { loop { if let Ok(v) = rx.try_recv() { drop(v); } std::thread::sleep(std::time::Duration::from_millis(1)); } }
```

_async poll loop with yield_now_

```rust
async fn f(q: &Queue) { loop { if q.try_recv().is_none() { tokio::task::yield_now().await; } } }
```

_try_lock loop with park_

```rust
fn f(m: &std::sync::Mutex<u8>) { loop { if let Ok(g) = m.try_lock() { drop(g); break; } std::thread::park(); } }
```

_blocking recv is not a spin_

```rust
fn f(rx: std::sync::mpsc::Receiver<u8>) { loop { let Ok(v) = rx.recv() else { break }; drop(v); } }
```

_plain computation loop_

```rust
fn f(mut n: u32) { while n > 0 { n -= 1; } }
```

_spin loop in test module_

```rust
#[cfg(test)]
mod tests {
    fn t(rx: std::sync::mpsc::Receiver<u8>) { loop { if let Ok(v) = rx.try_recv() { drop(v); } } }
}
```

### rust_catch_unwind

Require `// PANIC-BOUNDARY:` comment on `catch_unwind` calls.

> Catching a panic and continuing risks observing broken state. The comment must state the controlled-restart story.

|          |          |
| -------- | -------- |
| Severity | high     |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_full path catch_unwind_

```rust
fn f() { let _ = std::panic::catch_unwind(|| {}); }
```

_short path catch_unwind_

```rust
fn f() { let _ = panic::catch_unwind(|| {}); }
```

_method catch_unwind_

```rust
fn f<F: Future>(fut: F) { let _ = fut.catch_unwind(); }
```

**Good (passes):**

_catch_unwind with boundary comment_

```rust
fn f() {
    // PANIC-BOUNDARY: isolates one request; the worker restarts after any unwind
    let _ = std::panic::catch_unwind(|| {});
}
```

_catch_unwind in test module_

```rust
#[cfg(test)]
mod tests {
    fn t() { let _ = std::panic::catch_unwind(|| {}); }
}
```

_no catch_unwind_

```rust
fn f() -> Result<(), String> { Ok(()) }
```

### rust_cfg_not_test

Flag `#[cfg(not(test))]` — use dependency injection or feature flags instead.

> Code gated on #[cfg(not(test))] creates invisible production-only paths that are hard to test and reason about.

|          |           |
| -------- | --------- |
| Severity | medium    |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | no        |

**Bad (triggers violation):**

_cfg not test_

```rust
#[cfg(not(test))]
```

**Good (passes):**

_cfg test_

```rust
#[cfg(test)]
```

_normal cfg_

```rust
#[cfg(feature = "wasm")]
```

_comment with cfg not test_

```rust
// #[cfg(not(test))]
```

### rust_clone_in_loop

Flag `.clone()` and `.to_owned()` calls on loop-invariant receivers inside loop bodies.

> A loop-invariant ownership conversion may be avoidable or movable outside the loop. Borrow or restructure when possible. Conversions of values bound inside the loop are not reported because they commonly express a required per-item ownership transfer. This syntax-only rule cannot distinguish heap copies from cheap Arc, Rc, or Copy clones, so intentional cheap clones should be suppressed with a reason.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | no       |
| Fixable  | no       |

**Bad (triggers violation):**

_clone in while loop_

```rust
fn f(s: &String) { while true { let y = s.clone(); } }
```

_clone in loop loop_

```rust
fn f(s: &String) { loop { let y = s.clone(); break; } }
```

_to_owned in loop_

```rust
fn f(s: &str) { loop { let y = s.to_owned(); break; } }
```

**Good (passes):**

_clone of loop item varies by iteration_

```rust
fn f(v: Vec<String>) { for x in &v { let y = x.clone(); } }
```

_clone outside loop_

```rust
fn f(s: &String) { let y = s.clone(); }
```

_clone in test module_

```rust
#[cfg(test)]
mod tests {
    fn f(v: Vec<String>) { for x in &v { let y = x.clone(); } }
}
```

### rust_closure_dense_method_chain

Flag method-call chains containing at least the configured number of inline closure arguments.

> Closure-dense fluent chains hide several branching decisions in one expression. Name an intermediate result or extract a helper.

|                  |                  |
| ---------------- | ---------------- |
| Severity         | medium           |
| Type             | rust-ast         |
| Enabled          | no               |
| Fixable          | no               |
| Param: threshold | i64, default = 3 |

**Bad (triggers violation):**

_dense selection chain_

```rust
fn f(slots: &[Slot]) { let _ = slots.iter().enumerate().filter_map(|(index, slot)| ready(slot).then_some((index, slot))).min_by(|left, right| compare(left, right)).map(first).or_else(|| depleted(slots)); }
```

**Good (passes):**

_twenty uniform fluent calls_

```rust
fn f(w: Writer) { let _ = w.push_bind(1).push_bind(2).push_bind(3).push_bind(4).push_bind(5).push_bind(6).push_bind(7).push_bind(8).push_bind(9).push_bind(10).push_bind(11).push_bind(12).push_bind(13).push_bind(14).push_bind(15).push_bind(16).push_bind(17).push_bind(18).push_bind(19).push_bind(20); }
```

_long closure-free heterogeneous builder_

```rust
fn f(builder: Builder) { let _ = builder.name(1).cost(2).optional(true).retries(3).format(4).target(5).finish(); }
```

_one inline closure_

```rust
fn f(xs: &[i32]) { let _ = xs.iter().copied().map(|x| x + 1).collect::<Vec<_>>(); }
```

_two inline closures_

```rust
fn f(xs: &[i32]) { let _ = xs.iter().filter(|x| **x > 0).map(|x| x + 1).collect::<Vec<_>>(); }
```

_closure nested in a closure body is not an extra chain closure_

```rust
fn f(xs: Xs) { let _ = xs.map(|| ys.iter().filter(|y| ready(y))).inspect(noop).collect(); }
```

_closure in a nested method-call argument belongs to that nested chain_

```rust
fn f(xs: Xs) { let _ = xs.consume(factory.items().filter(|x| ready(x))).map(|x| value(x)); }
```

_closure in an unrelated base call is not counted_

```rust
fn f() { let _ = make(|| 1).iter().filter(|x| ready(x)).map(|x| value(x)); }
```

### rust_closure_param_position

Flag closure parameters that are not last, and fns taking more than one closure.

> Closures go last so multi-line closure arguments read naturally at call sites; more than one closure parameter makes calls unreadable and argument order ambiguous.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_closure before value param_

```rust
fn f(cb: impl Fn(), x: u32) {}
```

_generic closure before value param_

```rust
fn f<F: FnMut()>(cb: F, x: u32) {}
```

_where-bounded closure before value param_

```rust
fn f<F>(cb: F, x: u32) where F: Fn() -> u32 {}
```

_two closure params_

```rust
fn f(a: impl Fn(), b: impl FnOnce()) {}
```

**Good (passes):**

_closure last_

```rust
fn f(x: u32, cb: impl Fn()) {}
```

_where-bounded closure last_

```rust
fn f<F>(x: u32, cb: F) where F: FnOnce() {}
```

_fn pointer is not a closure_

```rust
fn f(cb: fn(), x: u32) {}
```

_no closure params_

```rust
fn f(x: u32, y: u32) {}
```

_closure first in test module_

```rust
#[cfg(test)]
mod tests {
    fn f(cb: impl Fn(), x: u32) {}
}
```

### rust_collection_new_in_loop

Flag collection constructors (`Vec::new()`, `vec![]`, `with_capacity`, ...) bound via `let` inside loops.

> Allocating a fresh collection per iteration is invisible overhead — hoist it out of the loop and .clear() each round.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | no       |
| Fixable  | no       |

**Bad (triggers violation):**

_Vec::new inside for loop, receiver-only use_

```rust
fn f(n: usize) { for _ in 0..n { let mut buf = Vec::new(); buf.push(1); } }
```

_String::new inside while loop_

```rust
fn f(n: usize) { let mut i = 0; while i < n { let mut s = String::new(); s.push('x'); i += 1; } }
```

_vec! macro inside loop_

```rust
fn f(n: usize) { for _ in 0..n { let v = vec![1, 2]; let _ = v.len(); } }
```

_with_capacity inside loop_

```rust
fn f(n: usize) { for _ in 0..n { let mut m = std::collections::HashMap::with_capacity(8); m.insert(1, 1); } }
```

_binding in nested block inside loop_

```rust
fn f(xs: &[u32]) { for x in xs { if *x > 0 { let mut v = Vec::new(); v.push(*x); } } }
```

_return escape still flagged (approximation: only call-argument use counts as escaping)_

```rust
fn f(n: usize) -> Vec<u32> { for _ in 0..n { let v = Vec::new(); if !v.is_empty() { return v; } } Vec::new() }
```

**Good (passes):**

_escapes as method-call argument into outer collection_

```rust
fn f(n: usize, out: &mut Vec<Vec<u32>>) { for i in 0..n { let mut row = Vec::new(); row.push(i as u32); out.push(row); } }
```

_escapes as plain function-call argument_

```rust
fn g(v: Vec<u32>) { drop(v); } fn f(n: usize) { for _ in 0..n { let v = Vec::new(); g(v); } }
```

_hoisted allocation cleared per iteration_

```rust
fn f(n: usize) { let mut buf = Vec::new(); for _ in 0..n { buf.push(1); buf.clear(); } }
```

_constructor outside any loop_

```rust
fn f() { let mut v = Vec::new(); v.push(1); }
```

_constructor in loop in test module_

```rust
#[cfg(test)]
mod tests {
    fn t(n: usize) { for _ in 0..n { let mut v = Vec::new(); v.push(1); } }
}
```

### rust_collection_trait_completeness

Require collection trait counterparts: `iter()` needs `impl IntoIterator for &T`, `iter_mut()` needs `impl IntoIterator for &mut T`, and `FromIterator`/`Extend` come in pairs.

> Collections missing the matching std iterator traits break for-loops over references and generic code expecting the standard surface.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_iter without IntoIterator for ref_

```rust
struct Bag(Vec<u32>);
impl Bag {
    fn iter(&self) -> std::slice::Iter<'_, u32> { self.0.iter() }
}
```

_iter_mut without IntoIterator for mut ref_

```rust
struct Bag(Vec<u32>);
impl Bag {
    fn iter_mut(&mut self) -> std::slice::IterMut<'_, u32> { self.0.iter_mut() }
}
```

_FromIterator without Extend_

```rust
struct Bag(Vec<u32>);
impl FromIterator<u32> for Bag {
    fn from_iter<I: IntoIterator<Item = u32>>(iter: I) -> Self { Bag(iter.into_iter().collect()) }
}
```

_Extend without FromIterator_

```rust
struct Bag(Vec<u32>);
impl Extend<u32> for Bag {
    fn extend<I: IntoIterator<Item = u32>>(&mut self, iter: I) { self.0.extend(iter) }
}
```

**Good (passes):**

_iter with IntoIterator for ref_

```rust
struct Bag(Vec<u32>);
impl Bag {
    fn iter(&self) -> std::slice::Iter<'_, u32> { self.0.iter() }
}
impl<'a> IntoIterator for &'a Bag {
    type Item = &'a u32;
    type IntoIter = std::slice::Iter<'a, u32>;
    fn into_iter(self) -> Self::IntoIter { self.0.iter() }
}
```

_iter_mut with IntoIterator for mut ref_

```rust
struct Bag(Vec<u32>);
impl Bag {
    fn iter_mut(&mut self) -> std::slice::IterMut<'_, u32> { self.0.iter_mut() }
}
impl<'a> IntoIterator for &'a mut Bag {
    type Item = &'a mut u32;
    type IntoIter = std::slice::IterMut<'a, u32>;
    fn into_iter(self) -> Self::IntoIter { self.0.iter_mut() }
}
```

_FromIterator with Extend_

```rust
struct Bag(Vec<u32>);
impl FromIterator<u32> for Bag {
    fn from_iter<I: IntoIterator<Item = u32>>(iter: I) -> Self { Bag(iter.into_iter().collect()) }
}
impl Extend<u32> for Bag {
    fn extend<I: IntoIterator<Item = u32>>(&mut self, iter: I) { self.0.extend(iter) }
}
```

_no collection surface_

```rust
struct Bag(Vec<u32>);
impl Bag {
    fn len(&self) -> usize { self.0.len() }
}
```

_iter in test module_

```rust
#[cfg(test)]
mod tests {
    struct Bag(Vec<u32>);
    impl Bag {
        fn iter(&self) -> std::slice::Iter<'_, u32> { self.0.iter() }
    }
}
```

_associated fn iter without receiver_

```rust
struct Gen;
impl Gen {
    fn iter() -> std::ops::Range<u32> { 0..4 }
}
```

_free fn iter_

```rust
fn iter() -> std::ops::Range<u32> { 0..4 }
```

### rust_comment_space

Require a space after `//` in comments (`//bad` -> `// good`).

> Missing space after // makes comments harder to read and looks like accidentally commented-out code.

|          |           |
| -------- | --------- |
| Severity | low       |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | yes       |

**Bad (triggers violation):**

_no space after //_

```rust
//comment
```

**Good (passes):**

_space after //_

```rust
// comment
```

_doc comment_

```rust
/// doc comment
```

_inner doc comment_

```rust
//! inner doc
```

_hash prefix_

```rust
//# header
```

_bare double slash_

```rust
//
```

_not a comment_

```rust
let x = 1;
```

### rust_commented_code

Detect blocks of commented-out code (2+ consecutive lines).

> Commented-out code is dead weight. Use version control to recover old code instead of leaving it inline.

|          |           |
| -------- | --------- |
| Severity | low       |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | no        |

**Bad (triggers violation):**

_two consecutive commented code lines_

```rust
// let x = 5;
// let y = 10;
```

**Good (passes):**

_single commented line passes_

```rust
// let x = 5;
```

_normal comments_

```rust
// This is a normal comment
// explaining the code below
```

_doc style comments_

```rust
// NOTE: this is important
// SAFETY: we checked bounds
```

_non-consecutive code comments_

```rust
// let x = 5;
let y = 10;
// let z = 15;
```

_mixed code and prose resets_

```rust
// let x = 5;
// This is a sentence about something.
// let y = 10;
```

_prose with for keyword_

```rust
// Each job can have multiple inputs (one for defaults, one for overrides)
// The order is determined by the configuration entries
```

_prose with self reference_

```rust
// Construct an OperationContext from &self.state + &mut self.sink.
// Used 10 times in the event loop; macro avoids repeating the struct literal.
```

### rust_concrete_io_param

Flag fn parameters typed as concrete I/O handles like `File` or `TcpStream`.

> Concrete I/O parameter types couple logic to one byte source; `impl std::io::Read`/`impl std::io::Write` works with files, sockets, and buffers alike.

|              |                                                                            |
| ------------ | -------------------------------------------------------------------------- |
| Severity     | low                                                                        |
| Type         | rust-ast                                                                   |
| Enabled      | no                                                                         |
| Fixable      | no                                                                         |
| Param: types | [String], default = ["File", "Stdin", "Stdout", "TcpStream", "UnixStream"] |

**Bad (triggers violation):**

_File parameter by value_

```rust
fn parse(file: File) {}
```

_File parameter by mutable reference_

```rust
fn parse(file: &mut File) {}
```

_TcpStream parameter_

```rust
fn read_frame(stream: TcpStream) {}
```

_fully qualified TcpStream parameter_

```rust
fn read_frame(stream: std::net::TcpStream) {}
```

_Stdout parameter_

```rust
fn log_to(out: Stdout) {}
```

_File in impl method_

```rust
struct S;
impl S {
    fn parse(&self, file: &File) {}
}
```

**Good (passes):**

_impl Read parameter_

```rust
fn parse(data: impl std::io::Read) {}
```

_byte slice parameter_

```rust
fn parse(data: &[u8]) {}
```

_File as return type is fine_

```rust
fn open(path: &std::path::Path) -> File { make(path) }
```

_unlisted type passes_

```rust
fn parse(reader: BufReader<u8>) {}
```

_File parameter in test module_

```rust
#[cfg(test)]
mod tests {
    fn parse(file: File) {}
}
```

_syntax tree parameter_

```rust
fn inspect(file: &syn::File) {}
```

### rust_const_fn_candidate

Flag syntactically simple functions worth evaluating as `const fn` candidates.

> Const eligibility depends on types, trait implementations, destructors, and the active toolchain. This rule only identifies candidates; the compiler must confirm any manual change.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_pure arithmetic function_

```rust
fn double(x: u32) -> u32 { x * 2 }
```

_function with if-else on params_

```rust
fn max_val(a: u32, b: u32) -> u32 { if a > b { a } else { b } }
```

_match with literal arms is const-eligible_

```rust
fn f(a: u32) -> u32 { match a { _ => 1 } }
```

_struct literal is const-eligible_

```rust
fn f() -> Foo { Foo { x: 1 } }
```

_index expression is const-eligible_

```rust
fn f(a: [u32; 2]) -> u32 { a[0] }
```

_field access is const-eligible_

```rust
fn f(p: Point) -> u32 { p.x }
```

_tuple is const-eligible_

```rust
fn f(a: u32, b: u32) -> (u32, u32) { (a, b) }
```

_cast is const-eligible_

```rust
fn f(a: u32) -> u8 { a as u8 }
```

_nested block is const-eligible_

```rust
fn f() -> u32 { { 1 } }
```

_return of literal is const-eligible_

```rust
fn f() -> u32 { return 1; }
```

**Good (passes):**

_already const fn_

```rust
const fn double(x: u32) -> u32 { x * 2 }
```

_function with method calls_

```rust
fn f(s: &str) -> usize { s.len() }
```

_function with macro call_

```rust
fn f() -> String { format!("hello") }
```

_const candidate in test_

```rust
#[cfg(test)]
mod tests {
    fn double(x: u32) -> u32 { x * 2 }
}
```

_function with loop_

```rust
fn f() -> u32 { let mut i = 0; while i < 10 { i += 1; } i }
```

_empty function_

```rust
fn f() {}
```

_unsafe function_

```rust
unsafe fn f(x: u32) -> u32 { x * 2 }
```

_function with where clause_

```rust
fn f<T>(x: T) -> T where T: Copy { x }
```

_function with impl Trait param_

```rust
fn f(x: impl Into<u32>) -> u32 { 1 }
```

_function call is not const-eligible_

```rust
fn f() -> u32 { g() }
```

_await is not const-eligible_

```rust
fn f() -> u32 { a.await }
```

### rust_const_needs_doc

Require a doc or line comment on private consts and statics holding literal values.

> Magic values need context: why the value was chosen and what depends on it.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | no       |
| Fixable  | no       |

**Bad (triggers violation):**

_undocumented const with numeric literal_

```rust
const TIMEOUT_SECS: u64 = 30;
```

_undocumented static with string literal_

```rust
static ENDPOINT: &str = "primary";
```

_literal nested in a call_

```rust
const RETRY: Duration = Duration::from_secs(30);
```

_pub(crate) const still needs a doc_

```rust
pub(crate) const TIMEOUT_SECS: u64 = 30;
```

**Good (passes):**

_doc comment explains the value_

```rust
/// Upstream aborts after thirty seconds.
const TIMEOUT_SECS: u64 = 30;
```

_line comment above explains the value_

```rust
// Matches the upstream timeout policy.
const TIMEOUT_SECS: u64 = 30;
```

_comment above the attribute_

```rust
// Fixture table kept verbatim.
#[rustfmt::skip]
const NAMES: &[&str] = &["a"];
```

_pub const is pub_api_docs territory_

```rust
pub const TIMEOUT_SECS: u64 = 30;
```

_no literal in initializer_

```rust
const SIZE: usize = std::mem::size_of::<u64>();
```

_const in test module_

```rust
#[cfg(test)]
mod tests {
    const TIMEOUT_SECS: u64 = 30;
}
```

_function-local const is not module-level_

```rust
fn f() -> u64 {
    const LOCAL: u64 = 30;
    LOCAL
}
```

### rust_conversion_self_convention

Enforce C-CONV receivers: `as_`/`to_` methods borrow (`&self`), `into_` methods consume (`self`).

> The `as_`/`to_`/`into_` prefixes promise a cost and ownership contract; a consuming `to_` or borrowing `into_` misleads every caller.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | no       |
| Fixable  | no       |

**Bad (triggers violation):**

_as\_ takes self by value_

```rust
struct S;
impl S {
    fn as_str(self) {}
}
```

_to\_ takes self by value_

```rust
struct S;
impl S {
    fn to_vec(self) {}
}
```

_into\_ borrows self_

```rust
struct S;
impl S {
    fn into_parts(&self) {}
}
```

_into\_ borrows self mutably_

```rust
struct S;
impl S {
    fn into_inner(&mut self) {}
}
```

_wrong convention on trait method_

```rust
trait T {
    fn as_bytes(self);
}
```

**Good (passes):**

_as\_ borrows_

```rust
struct S;
impl S {
    fn as_str(&self) {}
}
```

_as\_ borrows mutably_

```rust
struct S;
impl S {
    fn as_mut_slice(&mut self) {}
}
```

_to\_ borrows_

```rust
struct S;
impl S {
    fn to_vec(&self) {}
}
```

_into\_ consumes_

```rust
struct S;
impl S {
    fn into_parts(self) {}
}
```

_no receiver is exempt_

```rust
struct S;
impl S {
    fn into_config() -> S { S }
}
```

_non-conversion method_

```rust
struct S;
impl S {
    fn assemble(self) {}
}
```

_wrong convention in test module_

```rust
#[cfg(test)]
mod tests {
    struct S;
    impl S {
        fn as_str(self) {}
    }
}
```

### rust_ctor_new

Flag public structs with `Default` but no `pub fn new` — constructors are static inherent methods (C-CTOR).

> Users reach for X::new() first; a type offering only Default surprises them and breaks the upstream C-CTOR convention.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_derived Default without new_

```rust
#[derive(Default)]
pub struct Pool { size: u32 }
```

_manual Default without new_

```rust
pub struct Pool { size: u32 }
impl Default for Pool {
    fn default() -> Self {
        Pool { size: 0 }
    }
}
```

_private new only_

```rust
#[derive(Default)]
pub struct Pool { size: u32 }
impl Pool {
    fn new() -> Self {
        Pool { size: 0 }
    }
}
```

**Good (passes):**

_Default with pub new_

```rust
#[derive(Default)]
pub struct Pool { size: u32 }
impl Pool {
    pub fn new() -> Self {
        Pool { size: 0 }
    }
}
```

_no Default_

```rust
pub struct Pool { size: u32 }
```

_struct-literal constructible_

```rust
#[derive(Default)]
pub struct Point { pub x: f32, pub y: f32 }
```

_private struct_

```rust
#[derive(Default)]
struct Pool { size: u32 }
```

_Default without new in test module_

```rust
#[cfg(test)]
mod tests {
    #[derive(Default)]
    pub struct Pool { size: u32 }
}
```

### rust_ctor_param_count

Flag constructors with too many parameters or runs of identically-typed primitives — cascade construction through helper types.

> Long or same-typed constructor parameter lists invite silent argument mix-ups; grouping parameters semantically makes construction self-checking.

|                  |                  |
| ---------------- | ---------------- |
| Severity         | medium           |
| Type             | rust-ast         |
| Enabled          | no               |
| Fixable          | no               |
| Param: threshold | i64, default = 4 |

**Bad (triggers violation):**

_constructor with four parameters_

```rust
struct Deposit;
impl Deposit {
    pub fn new(account: Account, amount: Currency, memo: Memo, clock: Clock) -> Self {
        Deposit
    }
}
```

_three consecutive str parameters_

```rust
struct Deposit;
impl Deposit {
    pub fn new(bank: &str, customer: &str, currency: &str) -> Self {
        Deposit
    }
}
```

_with\_ constructor returning type name_

```rust
struct Deposit;
impl Deposit {
    pub fn with_parts(a: Account, b: Currency, c: Memo, d: Clock) -> Deposit {
        Deposit
    }
}
```

**Good (passes):**

_constructor with two parameters_

```rust
struct Deposit;
impl Deposit {
    pub fn new(account: Account, amount: Currency) -> Self {
        Deposit
    }
}
```

_mixed types below threshold_

```rust
struct Deposit;
impl Deposit {
    pub fn new(amount: u64, bank: &str, id: u32) -> Self {
        Deposit
    }
}
```

_non-constructor name_

```rust
struct Acc;
impl Acc {
    fn combine(a: u32, b: u32, c: u32, d: u32) -> u32 {
        a + b + c + d
    }
}
```

_chainable with\_ method has receiver_

```rust
struct Acc;
impl Acc {
    pub fn with_size(mut self, a: u32, b: u32, c: u32, d: u32) -> Self {
        self
    }
}
```

_trait impl constructor_

```rust
struct Acc;
trait Make {
    fn new_from(a: u32, b: u32, c: u32, d: u32) -> Self;
}
impl Make for Acc {
    fn new_from(a: u32, b: u32, c: u32, d: u32) -> Self {
        Acc
    }
}
```

_wide constructor in test module_

```rust
#[cfg(test)]
mod tests {
    struct Acc;
    impl Acc {
        pub fn new(a: u32, b: u32, c: u32, d: u32) -> Self {
            Acc
        }
    }
}
```

### rust_cyclomatic_complexity

Flag functions with cyclomatic complexity > threshold.

> High cyclomatic complexity means many execution paths, making the function hard to test and prone to bugs.

|                  |                   |
| ---------------- | ----------------- |
| Severity         | medium            |
| Type             | rust-ast          |
| Enabled          | no                |
| Fixable          | no                |
| Param: threshold | i64, default = 15 |

**Bad (triggers violation):**

_too many branches_

```rust
fn f(x: bool) { if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} }
```

_mixed constructs over threshold_

```rust
fn f(x: u8) -> u8 {
    match x { 0 => 1, 1 => 2, 2 => 3, 3 => 4, _ => 5 }
    for _ in it { }
    while a && b { }
    loop { break v && w; }
    let _ = c || d || e;
    let _ = |z: bool| if z { 1 } else { 2 };
    let _ = h()?;
    let _ = p && q;
    return x;
}
```

**Good (passes):**

_simple function_

```rust
fn f() { let x = 1; }
```

_moderate branching is fine_

```rust
fn f(x: bool) { if x {} if x {} if x {} if x {} if x {} if x {} }
```

_mixed constructs at threshold_

```rust
fn f(x: u8) -> u8 {
    match x { 0 => 1, 1 => 2, 2 => 3, 3 => 4, _ => 5 }
    for _ in it { }
    while a && b { }
    loop { break v && w; }
    let _ = c || d || e;
    let _ = |z: bool| if z { 1 } else { 2 };
    let _ = h()?;
    return x;
}
```

### rust_dbg

Ban `dbg!()` macro calls in production code.

> dbg!() writes to stderr and is meant for temporary debugging. Leaving it in production pollutes output.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | yes      |

**Bad (triggers violation):**

_dbg! left in code_

```rust
fn f() { dbg!(1); }
```

_production dbg with test module_

```rust
fn prod() { dbg!(1); }
#[cfg(test)]
mod tests {
    fn t() { dbg!(2); }
}
```

**Good (passes):**

_no dbg_

```rust
fn f() { let x = 1; }
```

_dbg in string literal_

```rust
fn f() { let s = "dbg!(value)"; }
```

_dbg in test module_

```rust
#[cfg(test)]
mod tests {
    fn t() { dbg!(1); }
}
```

### rust_deep_exit

Ban `std::process::exit()` in library code.

> process::exit() skips destructors and cleanup. Return Result from main instead so resources are released properly.

|          |          |
| -------- | -------- |
| Severity | high     |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_process::exit in library_

```rust
fn f() { std::process::exit(1); }
```

**Good (passes):**

_no exit call_

```rust
fn f() { let exit = 0; }
```

_exit in test module_

```rust
#[cfg(test)]
mod tests {
    fn t() { std::process::exit(1); }
}
```

_custom exit function_

```rust
fn f() { exit(0); }
```

### rust_deeply_nested_types

Flag type annotations with > 3 levels of generic nesting.

> Types like HashMap<String, Vec<Option<Arc<T>>>> are unreadable. Use type aliases to name intermediate types.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_depth 4 fails_

```rust
fn f(x: Vec<Vec<HashMap<String, Vec<i32>>>>) {}
```

_struct field with deep nesting_

```rust
struct S { x: Vec<Vec<HashMap<String, Vec<i32>>>> }
```

_return type with deep nesting_

```rust
fn f() -> Vec<Vec<HashMap<String, Vec<i32>>>> { todo!() }
```

_reference to depth 4 fails_

```rust
fn f(x: &Vec<Vec<Vec<Vec<i32>>>>) {}
```

_tuple element depth 4 fails_

```rust
fn f(x: (u8, Vec<Vec<Vec<Vec<i32>>>>)) {}
```

_array element depth 4 fails_

```rust
fn f(x: [Vec<Vec<Vec<Vec<i32>>>>; 4]) {}
```

_slice element depth 4 fails_

```rust
fn f(x: &[Vec<Vec<Vec<Vec<i32>>>>]) {}
```

_parenthesized type depth 4 fails_

```rust
fn f(x: (Vec<Vec<Vec<Vec<i32>>>>)) {}
```

**Good (passes):**

_shallow type_

```rust
fn f(x: Vec<i32>) {}
```

_depth 3 passes_

```rust
fn f(x: Vec<Vec<Vec<i32>>>) {}
```

_deep nesting in test module_

```rust
#[cfg(test)]
mod tests {
    fn f(x: Vec<Vec<HashMap<String, Vec<i32>>>>) {}
}
```

_reference to depth 3 passes_

```rust
fn f(x: &Vec<Vec<Vec<i32>>>) {}
```

### rust_default_hasher

Flag std `HashMap`/`HashSet` types and constructors that use the default SipHash hasher.

> SipHash buys DoS resistance that trusted internal keys do not need — a fast hasher (foldhash/FxHash) is significantly quicker.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | no       |
| Fixable  | no       |

**Bad (triggers violation):**

_HashMap<K, V> field with default hasher_

```rust
struct S { m: HashMap<u32, u32> }
```

_HashSet<T> parameter with default hasher_

```rust
fn f(s: HashSet<u32>) { drop(s); }
```

_std HashMap::new constructor_

```rust
fn f() { let mut m = std::collections::HashMap::new(); m.insert(1, 1); }
```

_HashSet::with_capacity constructor_

```rust
fn f() { let s: std::collections::HashSet<u64> = HashSet::with_capacity(8); drop(s); }
```

**Good (passes):**

_map with explicit hasher type param_

```rust
struct S { m: HashMap<u32, u32, FxBuildHasher> }
```

_set with explicit hasher type param_

```rust
struct S { s: HashSet<u32, FxBuildHasher> }
```

_fast-hasher crate alias_

```rust
fn f(m: foldhash::HashMap<u32, u32>) { drop(m); }
```

_turbofish constructor with explicit hasher_

```rust
fn f() { let _m = HashMap::<u32, u32, FxBuildHasher>::new(); }
```

_BTreeMap does not hash_

```rust
struct S { m: std::collections::BTreeMap<u32, u32> }
```

_default hasher in test module_

```rust
#[cfg(test)]
mod tests {
    fn t() { let _m: HashMap<u32, u32> = HashMap::new(); }
}
```

### rust_deny_warnings

Ban `#![deny(warnings)]` — breaks on compiler upgrades.

> deny(warnings) causes builds to break on compiler upgrades when new warnings are introduced. Use specific lint names.

|          |           |
| -------- | --------- |
| Severity | medium    |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | yes       |

**Bad (triggers violation):**

_deny warnings_

```rust
#![deny(warnings)]
```

**Good (passes):**

_deny unused_

```rust
#![deny(unused)]
```

_comment with deny warnings_

```rust
// #![deny(warnings)]
```

### rust_derive_order

Require traits inside derive attributes to be sorted alphabetically.

> Stable derive ordering keeps attribute diffs deterministic and makes duplicated traits easy to spot.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | yes      |

**Bad (triggers violation):**

_unsorted derives_

```rust
#[derive(Debug, Clone)]
struct Item;
```

_qualified paths use full text_

```rust
#[derive(thiserror::Error, Clone, Debug)]
enum Failure {}
```

**Good (passes):**

_sorted derives_

```rust
#[derive(Clone, Debug, thiserror::Error)]
struct Item;
```

_non-derive attribute_

```rust
#[cfg(test)]
struct Item;
```

### rust_dll_boundary_types

Flag `String`, `Vec`, `Box`, `dyn` objects, `TypeId`, and `Instant` in `extern "C"` signatures.

> Each Rust DLL has its own statics, type layouts, and type ids, so only `#[repr(C)]`-style, primitive, or raw-pointer data is portable across the boundary.

|          |          |
| -------- | -------- |
| Severity | high     |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_String parameter in foreign block_

```rust
extern "C" {
    fn take(s: String);
}
```

_Vec return from C export_

```rust
pub extern "C" fn give() -> Vec<u8> { Vec::new() }
```

_boxed slice parameter_

```rust
pub extern "C" fn boxed(b: Box<[u8]>) {}
```

_dyn trait reference parameter_

```rust
pub extern "C" fn cb(f: &dyn Fn()) {}
```

_TypeId parameter in foreign block_

```rust
extern "C" {
    fn id(t: TypeId);
}
```

_Instant parameter_

```rust
pub extern "C" fn when(t: std::time::Instant) {}
```

**Good (passes):**

_primitive and raw-pointer signature_

```rust
pub extern "C" fn ok(len: usize, data: *const u8) -> i32 { 0 }
```

_portable foreign block_

```rust
extern "C" {
    fn ok(x: u64) -> *mut u8;
}
```

_non-extern fn may use Rust types_

```rust
fn plain(s: String) {}
```

_wasm_bindgen files follow their own convention_

```rust
use wasm_bindgen::JsValue;
pub extern "C" fn f(s: String) {}
```

_C export in test module_

```rust
#[cfg(test)]
mod tests {
    pub extern "C" fn f(s: String) {}
}
```

### rust_doc_comment_period

Require doc comments to end with proper punctuation.

> Doc comments are sentences. Ending with punctuation keeps generated rustdoc consistent and professional. Not auto-fixable: appending a dot to a line that continues on the next line breaks the sentence — rephrase so each line is a complete sentence, or shorten the comment to one line.

|          |           |
| -------- | --------- |
| Severity | low       |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | no        |

**Bad (triggers violation):**

_missing period_

```rust
/// Returns the value
```

_inner doc missing period_

```rust
//! Module description
```

**Good (passes):**

_with period_

```rust
/// Returns the value.
```

_markdown header_

```rust
/// # Examples
```

_empty doc comment_

```rust
///
```

_code fence_

````rust
/// ```rust
````

_inner doc with period_

```rust
//! Module description.
```

_ends with backtick_

```rust
/// Returns `None`
```

_ends with colon_

```rust
/// The following:
```

### rust_doc_errors_section

Require a `# Errors` section on documented pub fns returning `Result`.

> Callers need failure conditions listed; canonical docs put them under `# Errors`.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_documented Result fn without Errors section_

```rust
/// Parses input.
pub fn f() -> Result<(), Error> { Ok(()) }
```

_Result type alias counts_

```rust
/// Reads bytes.
pub fn f() -> io::Result<()> { Ok(()) }
```

_impl fn without Errors section_

```rust
struct S;
impl S {
    /// Parses input.
    pub fn f(&self) -> Result<(), Error> { Ok(()) }
}
```

**Good (passes):**

_documented Result fn with Errors section_

```rust
/// Parses input.
///
/// # Errors
/// Fails on malformed input.
pub fn f() -> Result<(), Error> { Ok(()) }
```

_undocumented Result fn is pub_api_docs territory_

```rust
pub fn f() -> Result<(), Error> { Ok(()) }
```

_documented fn without Result_

```rust
/// Adds one.
pub fn f(x: u32) -> u32 { x }
```

_private documented Result fn_

```rust
/// Parses input.
fn f() -> Result<(), Error> { Ok(()) }
```

_impl fn with Errors section_

```rust
struct S;
impl S {
    /// Parses input.
    ///
    /// # Errors
    /// Fails on malformed input.
    pub fn f(&self) -> Result<(), Error> { Ok(()) }
}
```

_doc(hidden) has no doc text_

```rust
#[doc(hidden)]
pub fn f() -> Result<(), Error> { Ok(()) }
```

_test module fn_

```rust
#[cfg(test)]
mod tests {
    /// Parses input.
    pub fn f() -> Result<(), Error> { Ok(()) }
}
```

### rust_doc_inline_reexport

Require `#[doc(inline)]` on local re-exports and forbid it on external ones.

> Local re-exports should inline into module docs while external items stay visibly external.

|          |           |
| -------- | --------- |
| Severity | low       |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | no        |

**Bad (triggers violation):**

_local re-export without doc(inline)_

```rust
pub use crate::foo::Bar;
```

_self re-export without doc(inline)_

```rust
pub use self::foo::Bar;
```

_super re-export without doc(inline)_

```rust
pub use super::foo::Bar;
```

_inlined std re-export_

```rust
#[doc(inline)]
pub use std::fmt::Debug;
```

_same-line inlined core re-export_

```rust
#[doc(inline)] pub use core::fmt::Debug;
```

**Good (passes):**

_local re-export with doc(inline)_

```rust
#[doc(inline)]
pub use crate::foo::Bar;
```

_local re-export with doc(hidden)_

```rust
#[doc(hidden)]
pub use crate::internal::Bar;
```

_same-line attribute on local re-export_

```rust
#[doc(inline)] pub use crate::foo::Bar;
```

_std re-export without inline is correct_

```rust
pub use std::fmt::Debug;
```

_non-pub use is not a re-export_

```rust
use crate::foo::Bar;
```

_commented-out re-export_

```rust
// pub use crate::foo::Bar;
```

### rust_doc_panics_section

Require a `# Panics` section on documented pub fns that can panic.

> A documented fn that may panic must state when under `# Panics`.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | no       |
| Fixable  | no       |

**Bad (triggers violation):**

_documented fn with unwrap and no Panics section_

```rust
/// Reads the value.
pub fn f(x: Option<u32>) -> u32 { x.unwrap() }
```

_expect counts as a panic source_

```rust
/// Reads the value.
pub fn f(x: Option<u32>) -> u32 { x.expect("present") }
```

_panic macro counts_

```rust
/// Never returns.
pub fn f() { panic!("boom"); }
```

_assert macros count_

```rust
/// Validates input.
pub fn f(x: u32, y: u32) { assert_eq!(x, y, "mismatch"); }
```

_unreachable counts_

```rust
/// Dispatches.
pub fn f(x: bool) { if x { unreachable!(); } }
```

_impl fn with unwrap and no Panics section_

```rust
struct S;
impl S {
    /// Reads the value.
    pub fn f(&self, x: Option<u32>) -> u32 { x.unwrap() }
}
```

**Good (passes):**

_documented fn with unwrap and Panics section_

```rust
/// Reads the value.
///
/// # Panics
/// Panics when `x` is `None`.
pub fn f(x: Option<u32>) -> u32 { x.unwrap() }
```

_undocumented fn with unwrap_

```rust
pub fn f(x: Option<u32>) -> u32 { x.unwrap() }
```

_documented fn without panic sources_

```rust
/// Adds one.
pub fn f(x: u32) -> u32 { x.saturating_add(1) }
```

_debug_assert does not count_

```rust
/// Validates input.
pub fn f(x: bool) { debug_assert!(x, "must hold"); }
```

_private documented fn with unwrap_

```rust
/// Reads the value.
fn f(x: Option<u32>) -> u32 { x.unwrap() }
```

_unwrap inside nested item is not the fn's contract_

```rust
/// Delegates.
pub fn f() { fn inner() { None::<u32>.unwrap(); } }
```

_test module fn_

```rust
#[cfg(test)]
mod tests {
    /// Reads the value.
    pub fn f(x: Option<u32>) -> u32 { x.unwrap() }
}
```

### rust_doc_param_table

Ban `# Parameters`/`# Arguments`/`# Params` sections in doc comments.

> Rust docs explain parameters in prose, not tables; parameter sections duplicate the signature.

|          |           |
| -------- | --------- |
| Severity | low       |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | no        |

**Bad (triggers violation):**

_Parameters section_

```rust
/// # Parameters
```

_Arguments section_

```rust
/// # Arguments
```

_Params section at deeper level_

```rust
/// ## Params
```

_inner doc Arguments section_

```rust
//! # Arguments
```

_lowercase heading_

```rust
/// # arguments
```

**Good (passes):**

_Examples section_

```rust
/// # Examples
```

_Type Parameters heading is not a parameter table_

```rust
/// # Type Parameters
```

_heading inside code fence_

````rust
/// ```text
/// # Arguments
/// ```
````

_plain comment is not a doc comment_

```rust
// # Arguments
```

_prose mention of arguments_

```rust
/// Takes two arguments and merges them.
```

### rust_drop_panic

Ban `panic!`, `.unwrap()`, `.expect()` inside `impl Drop`.

> Panicking in Drop causes a double-panic abort. Drop must be infallible to avoid crashing the entire process.

|          |          |
| -------- | -------- |
| Severity | high     |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_panic in Drop_

```rust
struct Foo;
impl Drop for Foo {
  fn drop(&mut self) { panic!("oh no"); }
}
```

_unwrap in Drop_

```rust
struct Foo;
impl Drop for Foo {
  fn drop(&mut self) { Some(1).unwrap(); }
}
```

_expect in Drop_

```rust
struct Foo;
impl Drop for Foo {
  fn drop(&mut self) { Some(1).expect("msg"); }
}
```

_todo in Drop_

```rust
struct Foo;
impl Drop for Foo {
  fn drop(&mut self) { todo!(); }
}
```

**Good (passes):**

_println in Drop_

```rust
struct Foo;
impl Drop for Foo {
  fn drop(&mut self) { println!("dropping"); }
}
```

_panic outside Drop_

```rust
struct Foo;
impl Foo {
  fn bar(&self) { panic!("ok"); }
}
```

_panic in Drop in test module_

```rust
#[cfg(test)]
mod tests {
  struct Foo;
  impl Drop for Foo {
    fn drop(&mut self) { panic!("test"); }
  }
}
```

### rust_dup_expressions

Flag identical sub-expressions like `x == x`, `a - a`, `b && b`.

> Identical operands on both sides of an operator (x == x, a - a) are almost always copy-paste bugs.

|          |          |
| -------- | -------- |
| Severity | high     |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_x == x_

```rust
fn f(x: i32) { if x == x {} }
```

_a - a_

```rust
fn f(a: i32) { let _z = a - a; }
```

_b && b_

```rust
fn f(b: bool) { if b && b {} }
```

**Good (passes):**

_x == y_

```rust
fn f(x: i32, y: i32) { if x == y {} }
```

_a + b_

```rust
fn f(a: i32, b: i32) { let _z = a + b; }
```

_dup in test module_

```rust
#[cfg(test)]
mod tests {
    fn t(x: i32) { if x == x {} }
}
```

### rust_duplicate_strings

Find long string literals repeated across files; full-workspace runs are authoritative.

> Repeated long literals should have one named source of truth or a shared fixture.

|                        |                   |
| ---------------------- | ----------------- |
| Severity               | low               |
| Type                   | rust-workspace    |
| Enabled                | yes               |
| Fixable                | no                |
| Param: min_chars       | i64, default = 40 |
| Param: min_occurrences | i64, default = 3  |

**Bad (triggers violation):**

_long string repeated three times_

```rust
const A: &str = "a deliberately long repeated fixture string value";
const B: &str = "a deliberately long repeated fixture string value";
const C: &str = "a deliberately long repeated fixture string value";
```

**Good (passes):**

_only two occurrences_

```rust
const A: &str = "a deliberately long repeated fixture string value";
const B: &str = "a deliberately long repeated fixture string value";
```

_short strings are exempt_

```rust
const A: &str = "short"; const B: &str = "short"; const C: &str = "short";
```

### rust_duplicate_words

Flag repeated words in comments like `the the` or `is is`.

> Repeated words like 'the the' are typos that slip past spell checkers and make documentation look sloppy.

|          |           |
| -------- | --------- |
| Severity | low       |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | yes       |

**Bad (triggers violation):**

_the the in doc comment_

```rust
/// the the value
```

_is is in comment_

```rust
// is is
```

_case insensitive duplicate_

```rust
/// The the value
```

**Good (passes):**

_normal doc comment_

```rust
/// the value
```

_not a comment_

```rust
let the_the = 1;
```

### rust_dyn_wrapper_in_api

Flag `Rc<dyn …>`/`Arc<dyn …>`/`Box<dyn …>` in pub fn params, returns, and pub struct fields.

> Visible dyn wrappers lock APIs into object safety and infect user code; wrap trait objects in a private newtype.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_arc dyn parameter_

```rust
pub fn start(db: Arc<dyn Database>) {}
```

_box dyn parameter_

```rust
pub fn run(handler: Box<dyn Handler>) {}
```

_rc dyn pub field_

```rust
pub struct App {
    pub db: Rc<dyn Store>,
}
```

_nested arc dyn parameter_

```rust
pub fn start(db: Option<Arc<dyn Database>>) {}
```

_box dyn return_

```rust
pub fn handler() -> Box<dyn Handler> { make() }
```

**Good (passes):**

_box dyn error return_

```rust
pub fn run() -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
```

_box dyn error send sync_

```rust
pub fn run() -> Box<dyn Error + Send + Sync> { make() }
```

_private newtype wrapper_

```rust
pub struct DynStore(Arc<dyn Store>);
```

_private fn with dyn wrapper_

```rust
fn wire(db: Arc<dyn Database>) {}
```

_generic parameter instead_

```rust
pub fn start(db: impl Database) {}
```

_dyn wrapper in test module_

```rust
#[cfg(test)]
mod tests {
    pub fn start(db: Arc<dyn Database>) {}
}
```

### rust_error_missing_traits

Require `Display` and `std::error::Error` on public `*Error` types.

> std::error::Error mandates Display, and error types without both cannot participate in ?-chains, error reporting, or dyn Error composition.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_error struct without traits_

```rust
pub struct ParseError { line: usize }
```

_error struct with Display only_

```rust
pub struct ParseError;
impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("parse error")
    }
}
```

**Good (passes):**

_error struct with Display and Error_

```rust
pub struct ParseError;
impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("parse error")
    }
}
impl std::error::Error for ParseError {}
```

_thiserror derive on struct_

```rust
#[derive(Debug, thiserror::Error)]
#[error("parse failed")]
pub struct ParseError;
```

_thiserror derive on enum_

```rust
#[derive(Debug, Error)]
pub enum FetchError {
    #[error("io failed")]
    Io,
}
```

_private error struct_

```rust
struct ParseError;
```

_pub struct without error suffix_

```rust
pub struct Parser { pos: usize }
```

_error struct in test module_

```rust
#[cfg(test)]
mod tests {
    pub struct ParseError;
}
```

### rust_error_type_unit

Flag `Result<T, ()>` return types — use a real error type.

> Result<T, ()> discards error information. Use a real error type so callers can diagnose failures.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_Result with unit error_

```rust
fn f() -> Result<i32, ()> { Ok(1) }
```

_impl fn with unit error_

```rust
struct S;
impl S {
  fn f(&self) -> Result<(), ()> { Ok(()) }
}
```

**Good (passes):**

_Result with named error_

```rust
fn f() -> Result<i32, MyError> { Ok(1) }
```

_no return type_

```rust
fn f() { }
```

_non-Result return_

```rust
fn f() -> i32 { 1 }
```

_unit error in test module_

```rust
#[cfg(test)]
mod tests {
  fn f() -> Result<i32, ()> { Ok(1) }
}
```

### rust_excessive_float_precision

Flag float literals with more significant digits than the type can represent.

> Extra digits beyond what f32/f64 can represent are misleading. They suggest precision that does not exist.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_f32 with too many digits_

```rust
fn f() { let _x: f32 = 1.23456789012345_f32; }
```

_f64 with too many digits_

```rust
fn f() { let _x = 3.14159265358979323846_f64; }
```

**Good (passes):**

_f32 with ok precision_

```rust
fn f() { let _x: f32 = 1.234567_f32; }
```

_f64 with ok precision_

```rust
fn f() { let _x = 3.141592653589793_f64; }
```

_unsuffixed float is fine_

```rust
fn f() { let _x = 3.14159265358979323846; }
```

_excessive precision in test_

```rust
#[cfg(test)]
mod tests {
    fn t() { let _x = 1.23456789012345_f32; }
}
```

### rust_exotic_numeric_api

Flag `Saturating`/`Wrapping`/`NonZero*` in pub fn signatures.

> Std convention is plain numbers at public numeric boundaries; exotic wrappers belong to internal arithmetic.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_pub fn NonZero param_

```rust
pub fn window(n: std::num::NonZeroUsize) -> usize { n.get() }
```

_pub fn Wrapping return_

```rust
pub fn count() -> std::num::Wrapping<u32> { std::num::Wrapping(0) }
```

_pub fn Saturating param_

```rust
pub fn add(x: std::num::Saturating<u32>) -> u32 { x.0 }
```

_pub method NonZero param_

```rust
struct S;
impl S {
    pub fn set(&self, n: std::num::NonZeroU8) -> u8 { n.get() }
}
```

**Good (passes):**

_private fn NonZero param_

```rust
fn helper(n: std::num::NonZeroU8) -> u8 { n.get() }
```

_pub fn plain numbers_

```rust
pub fn window_size(n: usize) -> usize { n }
```

_exotic type only inside body_

```rust
pub fn f(n: u32) -> u32 { let w = std::num::Wrapping(n); w.0 }
```

_pub fn in test module_

```rust
#[cfg(test)]
mod tests {
    pub fn window(n: std::num::NonZeroUsize) -> usize { n.get() }
}
```

### rust_expect_message

Require `.expect()` to have a meaningful message, not generic ones.

> Generic expect messages like 'failed' give no context in panics. Describe what was expected and why.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_empty expect message_

```rust
fn f() { Some(1).expect(""); }
```

_generic expect message_

```rust
fn f() { Some(1).expect("failed"); }
```

**Good (passes):**

_good expect message_

```rust
fn f() { Some(1).expect("config file must exist at /etc/app.conf"); }
```

_expect in test module_

```rust
#[cfg(test)]
mod tests {
    fn t() { Some(1).expect(""); }
}
```

### rust_expect_over_allow

Flag `#[allow(...)]` in hand-written code — use `#[expect(..., reason = "...")]` instead.

> #[expect] warns when the suppressed lint no longer fires, preventing stale suppressions from accumulating; #[allow] silences forever (clippy analogue: allow_attributes).

|          |           |
| -------- | --------- |
| Severity | medium    |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | no        |

**Bad (triggers violation):**

_outer allow_

```rust
#[allow(dead_code)]
fn f() {}
```

_inner allow_

```rust
#![allow(clippy::too_many_arguments)]
```

_allow with comment still flagged_

```rust
#[allow(dead_code)] // webhook response fields
```

_allow after macro_rules closed_

```rust
macro_rules! m {
    () => {};
}
#[allow(unused)]
fn f() {}
```

**Good (passes):**

_expect with reason_

```rust
#[expect(dead_code, reason = "kept for wire format")]
fn f() {}
```

_allow inside macro_rules body_

```rust
macro_rules! m {
    () => {
        #[allow(unused)]
        fn f() {}
    };
}
```

_allow in comment_

```rust
// #[allow(dead_code)]
```

_allow in string literal_

```rust
fn f() { let s = "#[allow(dead_code)]"; }
```

### rust_fallible_in_iterator

Flag `.unwrap()`/`.expect()` inside iterator adapter closures.

> unwrap/expect inside iterator adapters panics mid-iteration with no recovery. Use filter*map or collect::<Result<*>>.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_unwrap in map closure_

```rust
fn f(v: Vec<Option<i32>>) -> Vec<i32> { v.into_iter().map(|x| x.unwrap()).collect() }
```

_expect in filter_map_

```rust
fn f(v: Vec<&str>) -> Vec<i32> { v.iter().filter_map(|x| Some(x.parse::<i32>().expect("bad"))).collect() }
```

_unwrap in for_each_

```rust
fn f(v: Vec<Option<i32>>) { v.iter().for_each(|x| { x.unwrap(); }); }
```

**Good (passes):**

_no unwrap in map_

```rust
fn f(v: Vec<i32>) -> Vec<i32> { v.into_iter().map(|x| x + 1).collect() }
```

_unwrap outside iterator_

```rust
fn f() { Some(1).unwrap(); }
```

_unwrap in iterator in test_

```rust
#[cfg(test)]
mod tests {
    fn t(v: Vec<Option<i32>>) { v.into_iter().map(|x| x.unwrap()); }
}
```

### rust_ffi_crate_naming

Require `-ffi` naming for crates exporting C symbols and `-sys` naming for crates linking foreign C items.

> The `-ffi` (export) and `-sys` (import) suffixes make a crate's FFI role immediately recognizable across projects.

|          |           |
| -------- | --------- |
| Severity | low       |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | no        |

**Bad (triggers violation):**

_no_mangle export in non-ffi crate_

```rust
#[no_mangle]
pub extern "C" fn service_tick() {}
```

_unsafe no_mangle export in non-ffi crate_

```rust
#[unsafe(no_mangle)]
pub extern "C" fn service_tick() {}
```

_foreign block in non-sys crate_

```rust
extern "C" {
    fn native_call();
}
```

**Good (passes):**

_plain rust fn_

```rust
pub fn tick() {}
```

_no_mangle without extern C fn_

```rust
#[no_mangle]
pub static VERSION: u32 = 1;
```

_extern C fn without no_mangle_

```rust
pub extern "C" fn callback() {}
```

_wasm_bindgen crate uses its own convention_

```rust
use wasm_bindgen::prelude::JsValue;
#[no_mangle]
pub extern "C" fn service_tick() {}
```

### rust_ffi_in_core

Flag `#[no_mangle] extern "C"` exports and `#[repr(C)]` raw-pointer structs in non-FFI crates.

> Business logic belongs in core crates as idiomatic safe Rust; interop concerns leaking into core force FFI ownership and data models onto everyone.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_no_mangle extern C fn in core crate_

```rust
#[no_mangle]
pub extern "C" fn service_tick() {}
```

_unsafe no_mangle extern C fn in core crate_

```rust
#[unsafe(no_mangle)]
pub extern "C" fn service_tick() {}
```

_repr(C) struct with raw-pointer field_

```rust
#[repr(C)]
pub struct Msg {
    data: *mut u8,
    len: usize,
}
```

**Good (passes):**

_repr(C) struct without pointers_

```rust
#[repr(C)]
pub struct Point {
    x: f32,
    y: f32,
}
```

_raw pointer without repr(C)_

```rust
pub struct Msg {
    data: *mut u8,
}
```

_extern C fn without no_mangle_

```rust
pub extern "C" fn callback() {}
```

_no_mangle without extern C abi_

```rust
#[no_mangle]
pub fn plain() {}
```

_wasm_bindgen files follow their own convention_

```rust
use wasm_bindgen::prelude::JsValue;
#[no_mangle]
pub extern "C" fn service_tick() {}
```

_export in test module_

```rust
#[cfg(test)]
mod tests {
    #[no_mangle]
    pub extern "C" fn service_tick() {}
}
```

### rust_ffi_thin_glue

Flag `extern "C"` functions in `*-ffi` crates whose body exceeds the line threshold.

> FFI glue only translates between C and Rust constructs; operational logic belongs in the core crate as idiomatic, safe, testable Rust.

|                  |                   |
| ---------------- | ----------------- |
| Severity         | low               |
| Type             | rust-ast          |
| Enabled          | yes               |
| Fixable          | no                |
| Param: threshold | i64, default = 25 |

**Bad (triggers violation):**

_extern C fn over the line threshold_

```rust
pub extern "C" fn business_logic() {
    let a01 = ();
    let a02 = ();
    let a03 = ();
    let a04 = ();
    let a05 = ();
    let a06 = ();
    let a07 = ();
    let a08 = ();
    let a09 = ();
    let a10 = ();
    let a11 = ();
    let a12 = ();
    let a13 = ();
    let a14 = ();
    let a15 = ();
    let a16 = ();
    let a17 = ();
    let a18 = ();
    let a19 = ();
    let a20 = ();
    let a21 = ();
    let a22 = ();
    let a23 = ();
    let a24 = ();
    let a25 = ();
    let a26 = ();
}
```

**Good (passes):**

_thin translation glue_

```rust
pub extern "C" fn transmit(data: *const u8, len: usize) -> u8 {
    match core_transmit(data, len) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}
```

_extern C fn at the line threshold_

```rust
pub extern "C" fn at_limit() {
    let a01 = ();
    let a02 = ();
    let a03 = ();
    let a04 = ();
    let a05 = ();
    let a06 = ();
    let a07 = ();
    let a08 = ();
    let a09 = ();
    let a10 = ();
    let a11 = ();
    let a12 = ();
    let a13 = ();
    let a14 = ();
    let a15 = ();
    let a16 = ();
    let a17 = ();
    let a18 = ();
    let a19 = ();
    let a20 = ();
    let a21 = ();
    let a22 = ();
    let a23 = ();
    let a24 = ();
}
```

_long plain fn is not glue_

```rust
fn core_logic() {
    let a01 = ();
    let a02 = ();
    let a03 = ();
    let a04 = ();
    let a05 = ();
    let a06 = ();
    let a07 = ();
    let a08 = ();
    let a09 = ();
    let a10 = ();
    let a11 = ();
    let a12 = ();
    let a13 = ();
    let a14 = ();
    let a15 = ();
    let a16 = ();
    let a17 = ();
    let a18 = ();
    let a19 = ();
    let a20 = ();
    let a21 = ();
    let a22 = ();
    let a23 = ();
    let a24 = ();
    let a25 = ();
    let a26 = ();
}
```

### rust_first_doc_sentence

Require the first doc sentence to end on the first line within a word budget.

> The first sentence becomes the rustdoc summary; long or spilling summaries break skimmable docs.

|                  |                   |
| ---------------- | ----------------- |
| Severity         | low               |
| Type             | rust-line         |
| Enabled          | no                |
| Fixable          | no                |
| Param: max_words | i64, default = 15 |

**Bad (triggers violation):**

_first sentence over the word budget_

```rust
/// This extremely long summary sentence keeps going and going with far too many words to fit the budget.
```

_first sentence spills onto the next line_

```rust
/// This summary continues
/// onto the next line.
```

_inner doc summary spills_

```rust
//! Module summary that drifts
//! across lines.
```

_empty first doc line delays the summary_

```rust
///
/// The summary arrives too late here.
```

**Good (passes):**

_short first sentence_

```rust
/// Returns the parsed value.
fn f() {}
```

_long detail after a short summary_

```rust
/// Short summary.
///
/// Detail sentences after the summary can be as long as they want to be without limits.
```

_inner doc short summary_

```rust
//! Module docs are covered.
```

_single line without terminator stays in budget_

```rust
/// Returns `None`
```

_sentence ends before a code fence_

````rust
/// Sums things.
/// ```
/// let total = sum(items). More words inside fences never matter at all here
/// ```
````

_heading-only block is skipped_

```rust
/// # Safety
```

_plain comments can ramble_

```rust
// this is not a doc comment so it can ramble on forever without any punctuation at all
```

_dotted names are not sentence ends_

```rust
/// Parses `foo.bar` fields quickly.
```

### rust_floating_point_eq

Flag direct `==`/`!=` comparison on `f32`/`f64` values.

> Floating-point equality is unreliable due to rounding. Use an epsilon comparison or relative tolerance instead.

|          |          |
| -------- | -------- |
| Severity | high     |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_direct f64 equality_

```rust
fn f(a: f64, b: f64) -> bool { a == b }
```

_direct f32 equality_

```rust
fn f(a: f32, b: f32) -> bool { a == b }
```

_f64 not-equal_

```rust
fn f(a: f64, b: f64) -> bool { a != b }
```

_compare float to literal zero_

```rust
fn f(a: f64) -> bool { a == 0.0 }
```

_local float binding equality_

```rust
fn f() -> bool { let x: f64 = 1.0; x == 0.0 }
```

_local float literal equality_

```rust
fn f() -> bool { let x = 1.0; x == 0.0 }
```

_left-hand-side float literal_

```rust
fn f(a: f64) -> bool { 0.0 == a }
```

_cast-to-float comparison_

```rust
fn f(b: i32) -> bool { b as f64 == 0 }
```

_float equality in impl method_

```rust
struct S; impl S { fn m(&self, a: f64, b: f64) -> bool { a == b } }
```

**Good (passes):**

_integer equality is fine_

```rust
fn f(a: i32, b: i32) -> bool { a == b }
```

_float comparison with epsilon_

```rust
fn f(a: f64, b: f64) -> bool { (a - b).abs() < f64::EPSILON }
```

_float eq in test module_

```rust
#[cfg(test)]
mod tests {
    fn t(a: f64, b: f64) -> bool { a == b }
}
```

_integer not-equal is fine_

```rust
fn f(a: u32, b: u32) -> bool { a != b }
```

### rust_foreign_reexports

Flag `pub use` re-exports of items from foreign crates.

> Re-exported foreign items blur type identity into aliases; users should depend on the defining crate directly.

|                |                        |
| -------------- | ---------------------- |
| Severity       | medium                 |
| Type           | rust-ast               |
| Enabled        | yes                    |
| Fixable        | no                     |
| Param: allowed | [String], default = [] |

**Bad (triggers violation):**

_pub use of foreign crate item_

```rust
pub use serde_json::Value;
```

_pub use re-exporting foreign crate root_

```rust
pub use rand;
```

_foreign item in use group_

```rust
pub use {std::fmt::Debug, chrono::Utc};
```

_renamed foreign re-export_

```rust
pub use serde_json::Value as JsonValue;
```

_grouped re-export from foreign crate_

```rust
pub use serde_json::{Map, Value};
```

**Good (passes):**

_pub use of crate item_

```rust
pub use crate::foo::Bar;
```

_pub use of self path_

```rust
pub use self::inner::Thing;
```

_pub use of std item_

```rust
pub use std::fmt::Debug;
```

_pub use of workspace crate_

```rust
pub use rulewright::RuleRegistry;
```

_non-pub use of foreign crate_

```rust
use serde_json::Value;
```

_pub(crate) use of foreign crate_

```rust
pub(crate) use serde_json::Value;
```

_doc(hidden) re-export_

```rust
#[doc(hidden)]
pub use serde_json::Value;
```

_re-export inside \_\_private module_

```rust
mod __private {
    pub use serde_json::Value;
}
```

_re-export inside \_private module_

```rust
mod _private {
    pub use serde_json::Value;
}
```

_re-export in test module_

```rust
#[cfg(test)]
mod tests {
    pub use serde_json::Value;
}
```

_re-export from local module_

```rust
mod inner {}
pub use inner::Thing;
```

_re-export from local out-of-line module_

```rust
pub mod infra;
pub use infra::Config;
```

_grouped re-export from local module_

```rust
mod inner {}
pub use inner::{Thing, Other};
```

### rust_from_instead_of_as

Flag `as` casts on suffixed literals — use `From`/`Into` instead.

> From/Into conversions are checked at compile time. 'as' casts silently truncate, which hides bugs.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | yes      |

**Bad (triggers violation):**

_suffixed literal cast_

```rust
fn f() { let x = 1u8 as u32; }
```

**Good (passes):**

_unsuffixed literal_

```rust
fn f() { let x = 1 as u32; }
```

_unknown source type_

```rust
fn f(a: u8) { let x = a as u32; }
```

_const context_

```rust
const X: u32 = 1u8 as u32;
```

_static context_

```rust
static X: u32 = 1u8 as u32;
```

_cast in test module_

```rust
#[cfg(test)]
mod tests {
    fn f() { let x = 1u8 as u32; }
}
```

### rust_future_send_assert

Require a compile-time `Send` assertion for every explicit `impl Future` in the same file.

> Explicitly declared futures silently turning !Send breaks Tokio and runtime-abstraction consumers; a const assertion catches the regression at compile time.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_Future impl without Send assertion_

```rust
struct Foo;
impl Future for Foo { type Output = (); }
```

_qualified Future impl without Send assertion_

```rust
struct Foo;
impl std::future::Future for Foo { type Output = (); }
```

_two implementors, one unasserted_

```rust
struct Foo;
struct Bar;
impl Future for Foo { type Output = (); }
impl Future for Bar { type Output = (); }
const fn assert_send<T: Send>() {}
const _: () = assert_send::<Foo>();
```

**Good (passes):**

_Future impl with Send assertion_

```rust
struct Foo;
impl Future for Foo { type Output = (); }
const fn assert_send<T: Send>() {}
const _: () = assert_send::<Foo>();
```

_no Future impl_

```rust
struct Foo;
impl Iterator for Foo { type Item = u8; }
```

_Future impl in test module_

```rust
#[cfg(test)]
mod tests {
    struct Foo;
    impl Future for Foo { type Output = (); }
}
```

### rust_getter_prefix

Flag methods named `get_something` — Rust getters are named after the field (C-GETTER).

> The `get_` prefix is noise: the std convention is `fn name(&self)`, with `get`/`get_mut` reserved for keyed or checked access.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | no       |
| Fixable  | no       |

**Bad (triggers violation):**

_get\_ method_

```rust
struct S;
impl S {
    fn get_name(&self) {}
}
```

_get\_ trait method_

```rust
trait T {
    fn get_len(&self) -> usize;
}
```

**Good (passes):**

_plain get passes_

```rust
struct S;
impl S {
    fn get(&self) {}
}
```

_get_mut passes_

```rust
struct S;
impl S {
    fn get_mut(&mut self) {}
}
```

_get_unchecked_mut passes_

```rust
struct S;
impl S {
    fn get_unchecked_mut(&mut self) {}
}
```

_get_or_insert_with passes_

```rust
struct S;
impl S {
    fn get_or_insert_with(&mut self) {}
}
```

_free fn is not a getter_

```rust
fn get_config() {}
```

_associated fn without receiver_

```rust
struct S;
impl S {
    fn get_default() -> S { S }
}
```

_getter in test module_

```rust
#[cfg(test)]
mod tests {
    struct S;
    impl S {
        fn get_name(&self) {}
    }
}
```

### rust_glob_reexport

Flag `pub use foo::*` glob re-exports outside platform-cfg'd HAL forwarding.

> Glob re-exports silently widen the public surface and are unreviewable in diffs; re-export items individually.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_pub glob reexport_

```rust
pub use internals::*;
```

_pub crate glob reexport_

```rust
pub(crate) use internals::*;
```

_glob inside group_

```rust
pub use internals::{helpers::*, Config};
```

**Good (passes):**

_private glob import_

```rust
fn f() { use internals::*; }
```

_named reexports_

```rust
pub use internals::{Config, Helper};
```

_windows hal forwarding_

```rust
#[cfg(windows)]
pub use windows_impl::*;
```

_target os hal forwarding_

```rust
#[cfg(target_os = "linux")]
pub use linux_impl::*;
```

_target arch hal forwarding_

```rust
#[cfg(target_arch = "wasm32")]
pub use wasm_impl::*;
```

_unix hal forwarding_

```rust
#[cfg(unix)]
pub use unix_impl::*;
```

_glob reexport in test module_

```rust
#[cfg(test)]
mod tests {
    pub use internals::*;
}
```

### rust_global_state

Flag `static` items with interior mutability and all `thread_local!` state.

> Mutable globals are secretly duplicated across linked crate versions and break test isolation; perf-only caches need a #rw suppression with reason.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_static atomic_

```rust
static COUNTER: AtomicUsize = AtomicUsize::new(0);
```

_static once lock_

```rust
static CACHE: OnceLock<String> = OnceLock::new();
```

_nested interior mutability_

```rust
static SLOTS: [Option<Mutex<u8>>; 2] = [None, None];
```

_thread local_

```rust
thread_local! {
    static TLS: RefCell<u8> = RefCell::new(0);
}
```

**Good (passes):**

_immutable str static_

```rust
static NAME: &str = "project";
```

_immutable numeric static_

```rust
static LIMIT: usize = 64;
```

_local mutex_

```rust
fn f() { let m = std::sync::Mutex::new(0); drop(m); }
```

_static atomic in test module_

```rust
#[cfg(test)]
mod tests {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
}
```

### rust_hardcoded_url

Flag hardcoded URLs in source code (should use config/env).

> Hardcoded URLs break when environments change. Use configuration or environment variables for host-specific URLs.

|          |           |
| -------- | --------- |
| Severity | medium    |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | no        |

**Bad (triggers violation):**

_hardcoded https URL_

```rust
fn f() { let url = "https://api.example.com/v1"; }
```

_hardcoded http URL_

```rust
fn f() { let url = "http://localhost:3000"; }
```

**Good (passes):**

_no URL_

```rust
fn f() { let x = 42; }
```

_URL in doc comment_

```rust
/// See https://docs.rs/foo for details.
```

_URL in regular comment_

```rust
// Reference: https://example.com/spec
```

### rust_impl_into_for_owned

Flag `impl Into<T> for X` — implement `From<X> for T` instead (gives Into for free).

> Implementing From<X> for T gives you Into<T> for X for free. Implementing Into directly is redundant and non-standard.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_impl Into directly_

```rust
struct Foo;
impl Into<String> for Foo { fn into(self) -> String { String::new() } }
```

**Good (passes):**

_impl From instead_

```rust
struct Foo;
impl From<Foo> for String { fn from(_: Foo) -> String { String::new() } }
```

_impl Into in test_

```rust
#[cfg(test)]
mod tests {
    struct Foo;
    impl Into<String> for Foo { fn into(self) -> String { String::new() } }
}
```

### rust_impl_member_order

Require inherent impl members to follow the canonical category and visibility order.

> Predictable inherent impl inventories keep construction and public APIs ahead of implementation details.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | no       |
| Fixable  | yes      |

**Bad (triggers violation):**

_constructor follows method_

```rust
struct Item;
impl Item {
    pub fn value(&self) {}
    /// Builds an item.
    #[cfg(test)]
    pub fn new() -> Result<Self, ()> { Ok(Self) }
}
```

_restricted method before public method_

```rust
struct Item;
impl Item {
    pub(crate) fn shared(&self) {}
    #[inline]
    pub fn visible(&self) {}
}
```

**Good (passes):**

_canonical member groups_

```rust
struct Item;
impl Item {
    const LIMIT: usize = 1;
    type Value = usize;
    pub fn new() -> Self { Self }
    pub fn value(&self) {}
    pub(crate) fn shared(&self) {}
    fn private(&self) {}
}
```

_trait impl is exempt_

```rust
struct Item;
impl Default for Item {
    fn default() -> Self { Self }
    const VALUE: usize = 1;
}
```

### rust_infallible_from_weak

Flag `impl From<weak>` next to fallible construction of the same type.

> An infallible conversion from a weak type bypasses the invariant the fallible constructor exists to guard; offer only TryFrom.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_From alongside TryFrom_

```rust
pub struct Month(u8);
impl TryFrom<u8> for Month {
    type Error = String;
    fn try_from(v: u8) -> Result<Self, String> { Err(String::new()) }
}
impl From<u8> for Month {
    fn from(v: u8) -> Self { Month(v) }
}
```

_From alongside inherent Result constructor_

```rust
pub struct Port(u16);
impl Port {
    pub fn new(v: u16) -> Result<Self, String> { Ok(Port(v)) }
}
impl From<u16> for Port {
    fn from(v: u16) -> Self { Port(v) }
}
```

_From from str alongside TryFrom_

```rust
pub struct Tag(String);
impl TryFrom<String> for Tag {
    type Error = String;
    fn try_from(v: String) -> Result<Self, String> { Err(v) }
}
impl From<&str> for Tag {
    fn from(v: &str) -> Self { Tag(v.to_string()) }
}
```

**Good (passes):**

_From without fallible construction_

```rust
pub struct Month(u8);
impl From<u8> for Month {
    fn from(v: u8) -> Self { Month(v) }
}
```

_From from strong type_

```rust
pub struct Inner(u8);
pub struct Outer(Inner);
impl TryFrom<u8> for Outer {
    type Error = String;
    fn try_from(v: u8) -> Result<Self, String> { Err(String::new()) }
}
impl From<Inner> for Outer {
    fn from(v: Inner) -> Self { Outer(v) }
}
```

_infallible constructor only_

```rust
pub struct Port(u16);
impl Port {
    pub fn new(v: u16) -> Self { Port(v) }
}
impl From<u16> for Port {
    fn from(v: u16) -> Self { Port(v) }
}
```

_From in test module_

```rust
#[cfg(test)]
mod tests {
    pub struct Month(u8);
    impl TryFrom<u16> for Month {
        type Error = String;
        fn try_from(v: u16) -> Result<Self, String> { Err(String::new()) }
    }
    impl From<u8> for Month {
        fn from(v: u8) -> Self { Month(v) }
    }
}
```

### rust_inherent_before_trait_impl

Require an inherent impl to precede trait impls for the same local type.

> Putting the type's own API first makes its primary behavior easier to discover.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_trait impl first_

```rust
struct Item;
impl Default for Item { fn default() -> Self { Self } }
impl Item { fn new() -> Self { Self } }
```

**Good (passes):**

_inherent impl first_

```rust
struct Item;
impl Item { fn new() -> Self { Self } }
impl Default for Item { fn default() -> Self { Self } }
```

_different types_

```rust
struct One; struct Two;
impl Default for One { fn default() -> Self { Self } }
impl Two {}
```

_trait impl only_

```rust
struct Item;
impl Default for Item { fn default() -> Self { Self } }
```

### rust_inline_test_module_size

Flag `#[cfg(test)] mod` blocks spanning more than threshold lines.

> Oversized inline test modules drown the business logic in the same file; tests touching only public API are integration tests and belong under `tests/`.

|                  |                    |
| ---------------- | ------------------ |
| Severity         | low                |
| Type             | rust-ast           |
| Enabled          | yes                |
| Fixable          | no                 |
| Param: threshold | i64, default = 200 |

**Good (passes):**

_small test module_

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn t() {}
}
```

_non-test module_

```rust
mod helpers {
    fn h() {}
}
```

### rust_large_async_local

Flag by-value `[T; N]` locals and parameters over threshold bytes inside async fns and blocks.

> Async locals and parameters embed in the future's state machine, inflating every task and spawn memcpy.

|                  |                     |
| ---------------- | ------------------- |
| Severity         | medium              |
| Type             | rust-ast            |
| Enabled          | yes                 |
| Fixable          | no                  |
| Param: threshold | i64, default = 1024 |

**Bad (triggers violation):**

_large repeat-literal array in async fn_

```rust
async fn f() { let buf = [0u8; 2048]; consume(&buf).await; }
```

_large annotated array local in async fn_

```rust
async fn f() { let buf: [u64; 512] = [0; 512]; consume(&buf).await; }
```

_large by-value array parameter on async fn_

```rust
async fn f(buf: [u8; 4096]) { consume(&buf).await; }
```

_large array local in async block_

```rust
fn g() { let _fut = async { let buf = [0u8; 2048]; consume(&buf).await; }; }
```

**Good (passes):**

_small array local in async fn_

```rust
async fn f() { let buf = [0u8; 128]; consume(&buf).await; }
```

_sync fn is rust_large_stack_array territory_

```rust
fn f() { let _buf = [0u8; 4096]; }
```

_array behind a reference parameter_

```rust
async fn f(buf: &[u8; 4096]) { consume(buf).await; }
```

_large array inside sync closure in async fn_

```rust
async fn f() { let g = || { let _buf = [0u8; 4096]; }; g(); }
```

_large async local in test module_

```rust
#[cfg(test)]
mod tests {
    async fn t() { let _buf = [0u8; 2048]; }
}
```

### rust_large_enum_variant

Flag enum variants that are much larger than others (should Box the large variant).

> All enum variants share the size of the largest. One huge variant wastes memory for every instance of the enum.

|                  |                    |
| ---------------- | ------------------ |
| Severity         | medium             |
| Type             | rust-ast           |
| Enabled          | yes                |
| Fixable          | no                 |
| Param: threshold | i64, default = 256 |

**Bad (triggers violation):**

_enum with large variant_

```rust
enum Msg { Small(u8), Large([u8; 1024]) }
```

_named-field large variant_

```rust
enum E { A { x: u8 }, B { data: [u64; 64] } }
```

_unit variant vs large variant_

```rust
enum E { Empty, Big([u8; 512]) }
```

**Good (passes):**

_enum with similar-sized variants_

```rust
enum Msg { A(u64), B(u64) }
```

_large variant boxed is fine_

```rust
enum Msg { Small(u8), Large(Box<[u8; 1024]>) }
```

_large enum in test_

```rust
#[cfg(test)]
mod tests {
    enum Msg { Small(u8), Large([u8; 1024]) }
}
```

_single variant enum_

```rust
enum Single { Only([u8; 1024]) }
```

### rust_large_fn_params

Flag functions with > threshold parameters.

> Functions with many parameters are hard to call correctly. Group related params into a struct.

|                  |                  |
| ---------------- | ---------------- |
| Severity         | medium           |
| Type             | rust-ast         |
| Enabled          | yes              |
| Fixable          | no               |
| Param: threshold | i64, default = 6 |

**Bad (triggers violation):**

_too many params_

```rust
fn f(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32) {}
```

**Good (passes):**

_few params_

```rust
fn f(a: i32, b: i32) {}
```

_self not counted_

```rust
struct S;
impl S {
    fn f(&self, a: i32, b: i32, c: i32, d: i32, e: i32, f: i32) {}
}
```

_at threshold_

```rust
fn f(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32) {}
```

### rust_large_stack_array

Flag large fixed-size arrays on the stack (>threshold bytes). WASM has limited stack.

> WASM has a fixed 1MB stack by default. Large stack arrays can silently overflow it and crash at runtime.

|                  |                     |
| ---------------- | ------------------- |
| Severity         | high                |
| Type             | rust-ast            |
| Enabled          | yes                 |
| Fixable          | no                  |
| Param: threshold | i64, default = 4096 |

**Bad (triggers violation):**

_large stack array_

```rust
fn f() { let _buf: [u8; 8192] = [0u8; 8192]; }
```

_u64 element array type_

```rust
struct S { buf: [u64; 1024] }
```

_boxed element array type_

```rust
struct S { buf: [Box<u32>; 1024] }
```

_tuple element array type_

```rust
struct S { buf: [(u64, u32); 1024] }
```

_reference element array type_

```rust
struct S<'a> { buf: [&'a u8; 1024] }
```

_f64 repeat literal array_

```rust
fn f() { let _b = [0.0f64; 1024]; }
```

_u64 repeat literal array_

```rust
fn f() { let _b = [0u64; 1024]; }
```

**Good (passes):**

_small stack array_

```rust
fn f() { let _buf = [0u8; 64]; }
```

_large array in test_

```rust
#[cfg(test)]
mod tests {
    fn t() { let _buf = [0u8; 8192]; }
}
```

### rust_log_in_loop

Flag logging macro invocations inside loop bodies in library code.

> Per-iteration telemetry turns hot loops into allocation and I/O hotspots; emit one batch-level event before or after the loop instead.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_info in for loop_

```rust
fn f(v: Vec<u32>) { for m in v { tracing::info!("processing"); } }
```

_warn in while loop_

```rust
fn f() { while running() { warn!("still waiting"); } }
```

_event in loop_

```rust
fn f() { loop { event!(Level::TRACE, "tick"); } }
```

_debug in nested block in loop_

```rust
fn f(v: Vec<u32>) { for m in v { if m > 0 { log::debug!("item"); } } }
```

**Good (passes):**

_log outside loop_

```rust
fn f() { tracing::info!("batch done"); }
```

_batch event before loop_

```rust
fn f(v: Vec<u32>) { info!("processing batch"); for m in v { handle(m); } }
```

_non-log macro in loop_

```rust
fn f(v: Vec<u32>) { for m in v { black_box!(m); } }
```

_log in loop in test module_

```rust
#[cfg(test)]
mod tests {
    fn f(v: Vec<u32>) { for m in v { tracing::info!("x"); } }
}
```

### rust_log_named_events

Flag `event!(...)` invocations without a `name:` argument before the level.

> Unnamed events cannot be grouped or filtered across log entries; name them `<component>.<operation>.<state>`.

|          |           |
| -------- | --------- |
| Severity | low       |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | no        |

**Bad (triggers violation):**

_unnamed event_

```rust
event!(Level::INFO, foo = 1, "msg");
```

_unnamed multi-line event_

```rust
event!(
    Level::INFO,
    file.path = p,
);
```

_name after level_

```rust
event!(Level::INFO, name: "a.b.c", "msg");
```

**Good (passes):**

_named event_

```rust
event!(name: "file.open.success", Level::INFO, "msg");
```

_named multi-line event_

```rust
tracing::event!(
    name: "a.b.c",
    Level::INFO,
);
```

_event in comment_

```rust
// event!(Level::INFO, "msg");
```

_event in string literal_

```rust
let s = "event!(Level::INFO)";
```

_other macro named event_

```rust
custom_event!(Level::INFO, "msg");
```

_non-event log macro_

```rust
info!("msg");
```

### rust_long_compound_name

Flag type definitions whose CamelCase name compounds more than threshold words.

> Rust item names are short: `AppConfig` over `GlobalApplicationConfig`; long compounds hide the item's essence.

|                  |                  |
| ---------------- | ---------------- |
| Severity         | low              |
| Type             | rust-ast         |
| Enabled          | yes              |
| Fixable          | no               |
| Param: threshold | i64, default = 4 |

**Bad (triggers violation):**

_five-word struct name_

```rust
struct GlobalApplicationConfigManagerImpl;
```

_five-word enum name_

```rust
enum BookingRequestQueueItemState { A }
```

_five-word trait name_

```rust
trait AsyncRemoteAccountDataSource {}
```

_five-word type alias_

```rust
type SharedRemoteAccountDataCache = ();
```

**Good (passes):**

_two-word name_

```rust
struct AppConfig;
```

_four words is at the threshold_

```rust
struct HtmlParserConfigBuilder;
```

_acronym run counts as one word_

```rust
struct HTMLParserConfigBuilder;
```

_long name in test module_

```rust
#[cfg(test)]
mod tests {
    struct GlobalApplicationConfigManagerFake;
}
```

### rust_loop_to_while

Flag `loop { if cond { break; } ... }` — use `while` instead.

> A loop with a break-if guard as the first statement is a while loop in disguise. while is clearer and less error-prone.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_loop with break-if at start_

```rust
fn f() { loop { if !cond() { break; } do_work(); } }
```

_loop with break-if-not at start_

```rust
fn f() { loop { if done() { break; } do_work(); } }
```

**Good (passes):**

_while loop is fine_

```rust
fn f() { while cond() { do_work(); } }
```

_loop with break in middle_

```rust
fn f() { loop { do_work(); if done() { break; } more_work(); } }
```

_loop with no break-if_

```rust
fn f() { loop { do_work(); break; } }
```

_break-if after a let statement_

```rust
fn f() { loop { let value = next(); if value == 0 { break; } } }
```

_break-if body has another statement_

```rust
fn f() { loop { if done() { note(); break; } do_work(); } }
```

_loop with break value_

```rust
fn f() -> i32 { loop { if done() { break 42; } } }
```

_loop in test module_

```rust
#[cfg(test)]
mod tests {
    fn f() { loop { if !cond() { break; } do_work(); } }
}
```

### rust_lossy_cast

Flag `as` casts to types that lose precision (`f32`, `u8`, `u16`, `i8`, `i16`).

> Casting to a smaller type (u64 as u8) silently truncates. Use try_into() to catch overflow at runtime.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_cast to u8_

```rust
fn f() { let x = 42u64 as u8; }
```

_cast to f32_

```rust
fn f() { let x = 1.0f64 as f32; }
```

_cast to i16_

```rust
fn f() { let x = 1000i32 as i16; }
```

**Good (passes):**

_cast to u32_

```rust
fn f() { let x = 42u64 as u32; }
```

_cast to u64_

```rust
fn f() { let x = 42u32 as u64; }
```

_cast to usize_

```rust
fn f() { let x = 42u32 as usize; }
```

_cast in test module_

```rust
#[cfg(test)]
mod tests {
    fn f() { let x = 42u64 as u8; }
}
```

### rust_macro_hidden_items

Flag fixed-name `pub` items emitted from quote! bodies.

> Fixed-name emitted items collide across expansions and clash with user code — interpolate the identifier instead.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_fixed-name pub struct in quote body_

```rust
fn expand() {
    let _ = quote::quote! { pub struct Generated; };
}
```

_fixed-name pub fn in quote body_

```rust
fn expand() {
    let _ = quote::quote! { pub fn helper() {} };
}
```

_fixed-name pub enum in quote body_

```rust
fn expand() {
    let _ = quote::quote! { pub enum Kind {} };
}
```

_fixed-name pub trait in quote body_

```rust
fn expand() {
    let _ = quote::quote! { pub trait Ext {} };
}
```

**Good (passes):**

_interpolated ident passes_

```rust
fn expand() {
    let _ = quote::quote! { pub struct #name; };
}
```

_repeated interpolation passes_

```rust
fn expand() {
    let _ = quote::quote! { #(pub fn #getters();)* };
}
```

_private emitted item passes_

```rust
fn expand() {
    let _ = quote::quote! { struct Hidden; };
}
```

_pub item outside quote body_

```rust
pub struct Normal;
```

_opener in comment_

```rust
// quote! { pub struct Generated; }
```

### rust_magic_numbers

Flag numeric literals outside the configured allowlist for review.

> Unnamed numeric literals can obscure intent. Use a named constant when its name explains the value; otherwise tune the allowlist or scope to match the project's policy.

|                |                                              |
| -------------- | -------------------------------------------- |
| Severity       | low                                          |
| Type           | rust-ast                                     |
| Enabled        | no                                           |
| Fixable        | no                                           |
| Param: allowed | [String], default = ["0", "1", "0.0", "1.0"] |

**Bad (triggers violation):**

_magic number in function_

```rust
fn f() { let x = 42; }
```

_power of two is magic_

```rust
fn f() { let x = 256; }
```

_small int is magic_

```rust
fn f() { let x = 2; }
```

_round number is magic_

```rust
fn f() { let x = 100; }
```

_float magic number_

```rust
fn f() { let x = 3.14; }
```

_negative magic number_

```rust
fn f() { let x = -42; }
```

_underscored literal is magic_

```rust
fn f() { let x = 1_000; }
```

_hex literal is magic_

```rust
fn f() { let x = 0xff; }
```

_literal in match arm body remains magic_

```rust
fn f(x: i32) -> i32 { match x { 0 => 42, _ => 1 } }
```

**Good (passes):**

_zero allowed by default_

```rust
fn f() { let x = 0; }
```

_one allowed by default_

```rust
fn f() { let x = 1; }
```

_const passes_

```rust
const N: i32 = 42;
```

_static passes_

```rust
static N: i32 = 42;
```

_enum discriminant passes_

```rust
enum E { A = 42 }
```

_float zero allowed by default_

```rust
fn f() { let x = 0.0; }
```

_magic number in test module_

```rust
#[cfg(test)]
mod tests {
    fn f() { let x = 42; }
}
```

_negative one allowed by default_

```rust
fn f() { let x = -1; }
```

_integer match patterns are not expressions_

```rust
fn f(x: i32) { match x { 2 | 42 => {}, _ => {} } }
```

_negative integer match pattern is not an expression_

```rust
fn f(x: i32) { match x { -42 => {}, _ => {} } }
```

### rust_manual_async_fn

Flag non-async functions that return `impl Future` by wrapping the whole body in one `async` block.

> `async fn` reads normally and avoids signature noise; explicit `impl Future` returns are only warranted in traits or for hot-path size control.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_impl Future wrapping one async block_

```rust
fn f() -> impl Future<Output = u8> { async { 1 } }
```

_qualified Future with async move block_

```rust
fn f() -> impl std::future::Future<Output = ()> { async move { } }
```

_inherent impl method_

```rust
struct S;
impl S { fn f() -> impl Future<Output = ()> { async { } } }
```

**Good (passes):**

_async fn_

```rust
async fn f() -> u8 { 1 }
```

_body does more than one async block_

```rust
fn f() -> impl Future<Output = u8> { let x = 1; async move { x } }
```

_trait declaration_

```rust
trait T { fn f() -> impl Future<Output = ()>; }
```

_trait default body_

```rust
trait T { fn f() -> impl Future<Output = ()> { async { } } }
```

_trait impl block_

```rust
struct S;
impl T for S { fn f() -> impl Future<Output = ()> { async { } } }
```

_non-Future impl trait return_

```rust
fn f() -> impl Iterator<Item = u8> { std::iter::once(1) }
```

_manual future in test module_

```rust
#[cfg(test)]
mod tests {
    fn f() -> impl Future<Output = ()> { async { } }
}
```

### rust_manual_error_impl

Reject hand-written `Display` and `Error` implementations for `*Error` types.

> Canonical errors derive `thiserror::Error` so formatting and source propagation remain declarative and consistent.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_manual display_

```rust
struct ParseError; impl std::fmt::Display for ParseError { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { todo!() } }
```

_manual error_

```rust
struct ParseError; impl std::error::Error for ParseError {}
```

**Good (passes):**

_thiserror_

```rust
#[derive(Debug, thiserror::Error)] #[error("bad")] struct ParseError;
```

_non-error display_

```rust
struct Label; impl std::fmt::Display for Label { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { todo!() } }
```

_unrelated trait_

```rust
struct ParseError; impl Clone for ParseError { fn clone(&self) -> Self { Self } }
```

_test error_

```rust
#[cfg(test)] mod tests { struct StubError; impl std::error::Error for StubError {} }
```

### rust_map_err_pure_wrap

Flag `.map_err(...)` that only wraps the error in another type — implement `From` and let `?` convert.

> Context-free error wrapping repeated at every call site obscures the happy path; a single From impl gives the conversion to every ? for free.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_bare variant path_

```rust
fn f(r: Result<u8, IoError>) -> Result<u8, AppError> { r.map_err(AppError::Io) }
```

_bare from path_

```rust
fn f(r: Result<u8, IoError>) -> Result<u8, AppError> { r.map_err(AppError::from) }
```

_closure wrapping only the error_

```rust
fn f(r: Result<u8, IoError>) -> Result<u8, AppError> { r.map_err(|e| AppError::Io(e)) }
```

**Good (passes):**

_closure adding context arguments_

```rust
fn f(r: Result<u8, IoError>) -> Result<u8, AppError> { r.map_err(|e| AppError::io(e, "config.toml")) }
```

_closure building struct context_

```rust
fn f(r: Result<u8, IoError>) -> Result<u8, AppError> { r.map_err(|e| AppError { source: e }) }
```

_closure transforming the error_

```rust
fn f(r: Result<u8, IoError>) -> Result<u8, AppError> { r.map_err(|e| AppError::parse(e.to_string())) }
```

_closure discarding the error_

```rust
fn f(r: Result<u8, IoError>) -> Result<u8, AppError> { r.map_err(|_| AppError::Unknown) }
```

_free fn argument_

```rust
fn f(r: Result<u8, IoError>) -> Result<u8, AppError> { r.map_err(convert) }
```

_no map_err_

```rust
fn f(r: Result<u8, IoError>) -> Result<u8, IoError> { r }
```

_pure wrap in test module_

```rust
#[cfg(test)]
mod tests {
    fn f(r: Result<u8, IoError>) -> Result<u8, AppError> { r.map_err(AppError::Io) }
}
```

### rust_match_layout

Keep match arms and `matches!` patterns visually structured.

> Complex pattern alternatives and dense multiline arms hide control flow; explicit match arms and whitespace make cases scannable.

|               |                                                                  |
| ------------- | ---------------------------------------------------------------- |
| Severity      | low                                                              |
| Type          | rust-ast                                                         |
| Enabled       | yes                                                              |
| Fixable       | no                                                               |
| Param: checks | [String], default = ["complex-matches", "multiline-arm-spacing"] |

**Bad (triggers violation):**

_complex multiline matches macro_

```rust
fn valid(pair: (Check, Fix)) -> bool {
    matches!(
        pair,
        (
            Check::Line,
            None | Some(Fix::Line),
        )
            | (Check::Ast, Some(Fix::Ast))
            | (Check::Toml, Some(Fix::Toml))
    )
}
```

_adjacent multiline match arms_

```rust
fn run(value: Value) {
    match value {
        Value::One => {
            prepare_one();
            one();
            finish_one();
        }
        Value::Two => {
            prepare_two();
            two();
            finish_two();
        }
    }
}
```

**Good (passes):**

_simple matches macro_

```rust
fn valid(value: Value) -> bool { matches!(value, Value::One | Value::Two) }
```

_separated multiline match arms_

```rust
fn run(value: Value) {
    match value {
        Value::One => {
            prepare_one();
            one();
            finish_one();
        }

        Value::Two => {
            prepare_two();
            two();
            finish_two();
        }
    }
}
```

_compact arms_

```rust
fn value(input: bool) -> usize { match input { true => 1, false => 0 } }
```

### rust_max_fn_lines

Flag functions longer than threshold lines.

> Functions over 150 lines are hard to understand, test, and review. Break them into smaller focused functions.

|                  |                    |
| ---------------- | ------------------ |
| Severity         | medium             |
| Type             | rust-ast           |
| Enabled          | yes                |
| Fixable          | no                 |
| Param: threshold | i64, default = 150 |

**Good (passes):**

_short function_

```rust
fn f() { let x = 1; }
```

### rust_max_nesting

Flag nesting depth > threshold levels.

> Deeply nested code is hard to follow. Use early returns, guard clauses, or extract helper functions.

|                  |                  |
| ---------------- | ---------------- |
| Severity         | medium           |
| Type             | rust-ast         |
| Enabled          | yes              |
| Fixable          | no               |
| Param: threshold | i64, default = 5 |

**Bad (triggers violation):**

_match/loop/else nested to depth 6 fails_

```rust
fn f() {
    loop {
        while c {
            if a {
            } else {
                match x {
                    _ => loop {
                        match y {
                            _ => loop { }
                        }
                    }
                }
            }
        }
    }
}
```

**Good (passes):**

_shallow function_

```rust
fn f() { if true { if true { } } }
```

_match/loop nested to depth 5 passes_

```rust
fn f() {
    loop {
        while c {
            match x {
                _ => loop {
                    match y {
                        _ => ()
                    }
                }
            }
        }
    }
}
```

### rust_mem_forget

Require `LEAK` or `SAFETY` comment on `std::mem::forget()` calls.

> mem::forget permanently leaks memory. A justification comment proves the leak is intentional, not a bug.

|          |          |
| -------- | -------- |
| Severity | high     |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_bare forget without comment_

```rust
fn f() { std::mem::forget(String::new()); }
```

**Good (passes):**

_forget with LEAK comment_

```rust
fn f() {
    // LEAK: intentionally leaked for static lifetime
    std::mem::forget(String::new());
}
```

_forget with SAFETY comment_

```rust
fn f() {
    // SAFETY: ownership transferred to FFI
    std::mem::forget(String::new());
}
```

_forget in test module_

```rust
#[cfg(test)]
mod tests {
    fn t() { std::mem::forget(String::new()); }
}
```

_unrelated forget function_

```rust
fn f() { cache::forget(key); }
```

### rust_missing_assert_message

Require a message argument on `assert!`, `assert_eq!`, `assert_ne!`.

> Assertions without messages produce opaque failures. A message explains what invariant was violated.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_assert without message_

```rust
fn f() { assert!(true); }
```

_assert_eq without message_

```rust
fn f() { assert_eq!(1, 2); }
```

_assert_ne without message_

```rust
fn f() { assert_ne!(1, 1); }
```

**Good (passes):**

_assert with message_

```rust
fn f() { assert!(true, "reason"); }
```

_assert_eq with message_

```rust
fn f() { assert_eq!(1, 2, "values differ"); }
```

_assert_ne with message_

```rust
fn f() { assert_ne!(1, 1, "should differ"); }
```

_assert in test module_

```rust
#[cfg(test)]
mod tests {
  fn t() { assert!(true); }
}
```

### rust_missing_capacity

Flag collections built with `new()`/`default()` then grown inside a loop over a sized source.

> When the final size is knowable at construction, with_capacity or collect avoids repeated reallocation and copying.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_Vec::new filled from xs.iter()_

```rust
fn f(xs: &[u32]) { let mut out = Vec::new(); for x in xs.iter() { out.push(*x); } }
```

_HashMap::new filled from &v_

```rust
fn f(v: Vec<u32>) { let mut m = std::collections::HashMap::new(); for x in &v { m.insert(*x, 1); } }
```

_String::new filled over a range_

```rust
fn f(n: usize) { let mut s = String::new(); for _ in 0..n { s.push_str("x"); } }
```

_annotated Default::default() filled in sized loop_

```rust
fn f(xs: &[u32]) { let mut out: Vec<u32> = Default::default(); for x in xs.iter() { out.push(*x); } }
```

_push nested deeper in loop body_

```rust
fn f(xs: &[u32]) { let mut out = Vec::new(); for x in xs.iter() { if *x > 0 { out.push(*x); } } }
```

**Good (passes):**

_with_capacity already used_

```rust
fn f(xs: &[u32]) { let mut out = Vec::with_capacity(xs.len()); for x in xs.iter() { out.push(*x); } }
```

_consecutive pushes belong to rust_vec_init_then_push, not this rule_

```rust
fn f() { let mut v = Vec::new(); v.push(1); v.push(2); }
```

_adapter chain has no knowable length_

```rust
fn f(xs: &[u32]) { let mut out = Vec::new(); for x in xs.iter().filter(|x| **x > 0) { out.push(**x); } }
```

_collect already sizes via size_hint_

```rust
fn f(xs: &[u32]) -> Vec<u32> { xs.iter().map(|x| x + 1).collect() }
```

_sized fill loop in test module_

```rust
#[cfg(test)]
mod tests {
    fn t(xs: &[u32]) { let mut out = Vec::new(); for x in xs.iter() { out.push(*x); } }
}
```

### rust_missing_debug

Require `#[derive(Debug)]` on public structs and enums.

> Public types without Debug are hard to inspect during development and cannot be used in assert messages.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | yes      |

**Bad (triggers violation):**

_pub struct without Debug_

```rust
pub struct Foo {}
```

_pub enum without Debug_

```rust
pub enum E { A, B }
```

**Good (passes):**

_pub struct with Debug_

```rust
#[derive(Debug)]
pub struct Foo {}
```

_private struct_

```rust
struct Foo {}
```

_pub(crate) struct_

```rust
pub(crate) struct Foo {}
```

_pub enum with Debug_

```rust
#[derive(Debug)]
pub enum E { A, B }
```

_pub struct in test module_

```rust
#[cfg(test)]
mod tests {
  pub struct Foo {}
}
```

_derive with other traits_

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct Foo {}
```

_manual Debug impl_

```rust
pub struct Bar {}
impl std::fmt::Debug for Bar {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Bar").finish()
  }
}
```

### rust_missing_error_context

Flag `.map_err(|_| ...)` that discards the original error.

> Discarding the original error in map*err(|*| ...) destroys the root cause, making failures hard to diagnose.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_wildcard discard_

```rust
fn f() { let _ = Ok::<i32, i32>(1).map_err(|_| "bad"); }
```

_underscore-prefixed param_

```rust
fn f() { let _ = Ok::<i32, i32>(1).map_err(|_e| "bad"); }
```

**Good (passes):**

_named param_

```rust
fn f() { let _ = Ok::<i32, i32>(1).map_err(|e| format!("{e}")); }
```

_function ref_

```rust
fn f() { let _ = Ok::<i32, String>(1).map_err(String::from); }
```

_discard in test module_

```rust
#[cfg(test)]
mod tests {
    fn t() { let _ = Ok::<i32, i32>(1).map_err(|_| "bad"); }
}
```

### rust_mod_order

Require contiguous module-declaration blocks to be alphabetically sorted.

> Stable module ordering makes module inventories predictable without grouping by visibility.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | yes      |

**Bad (triggers violation):**

_visibility does not affect ordering_

```rust
pub mod zebra;
mod alpha;
```

**Good (passes):**

_sorted declarations_

```rust
mod alpha;
pub mod beta;
mod gamma;
```

_separate declaration blocks_

```rust
mod zebra;
fn boundary() {}
mod alpha;
```

_inline modules are boundaries_

```rust
mod zebra {}
mod alpha;
```

_raw identifiers sort by their semantic name_

```rust
mod parser;
mod r#trait;
mod writer;
```

### rust_module_docs

Require `//!` module docs at the top of `lib.rs` and `mod.rs` files.

> Module docs are the entry point for API navigation; each module file should say what it contains.

|          |           |
| -------- | --------- |
| Severity | medium    |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | no        |

**Bad (triggers violation):**

_no module docs_

```rust
pub fn f() {}
```

_regular comment before module docs_

```rust
// a stray comment
//! Module docs.
pub fn f() {}
```

_only items_

```rust
pub struct S;
```

**Good (passes):**

_module docs on first line_

```rust
//! Module docs.

pub fn f() {}
```

_suppression directive before module docs_

```rust
// #rw(file: rust_panic) tooling fixture

//! Module docs.
pub fn f() {}
```

_inner attribute before module docs_

```rust
#![allow(dead_code)]
//! Module docs.
pub fn f() {}
```

_multi-line inner attribute before module docs_

```rust
#![allow(
    dead_code
)]
//! Module docs.
pub fn f() {}
```

_blank lines before module docs_

```rust


//! Module docs.
pub fn f() {}
```

### rust_module_prefix_in_name

Flag pub type definitions whose name repeats the module name as a prefix (`FooId` in `foo.rs`).

> Module-qualified APIs can avoid repeating the module in every public type. Flat re-exports and collision-prone APIs are intentional exceptions because this syntax-only rule cannot inspect the final exported namespace.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | no       |
| Fixable  | no       |

**Bad (triggers violation):**

_pub struct repeats module name_

```rust
pub struct FooId;
```

_pub enum repeats module name_

```rust
pub enum FooKind { A }
```

_pub trait repeats module name_

```rust
pub trait FooLike {}
```

_pub type alias repeats module name_

```rust
pub type FooResult = ();
```

**Good (passes):**

_name equal to module name_

```rust
pub struct Foo;
```

_prefix is not a whole segment_

```rust
pub struct Food;
```

_unrelated name_

```rust
pub struct Bar;
```

_private item exempt_

```rust
struct FooId;
```

_prefixed type in test module_

```rust
#[cfg(test)]
mod tests {
    pub struct FooFixture;
}
```

### rust_multiple_inherent_impl

Flag multiple `impl Foo` blocks for the same type in one file.

> Split impl blocks for the same type scatter related methods. Keep them in one block for discoverability.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_duplicate impl blocks_

```rust
struct Foo;
impl Foo { fn a() {} }
impl Foo { fn b() {} }
```

**Good (passes):**

_different types_

```rust
struct Foo;
struct Bar;
impl Foo {}
impl Bar {}
```

_trait impl not counted_

```rust
struct Foo;
impl Foo {}
impl std::fmt::Display for Foo { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { Ok(()) } }
```

_duplicate impl in test module_

```rust
#[cfg(test)]
mod tests {
  struct Foo;
  impl Foo { fn a() {} }
  impl Foo { fn b() {} }
}
```

### rust_mutex_in_async

Flag `std::sync::Mutex` usage in async functions under an async-mutex-only policy.

> Repositories that prohibit blocking mutexes in async code can opt into an explicit scheduler policy.

|          |          |
| -------- | -------- |
| Severity | high     |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_std Mutex in async fn_

```rust
async fn f() { let _m = std::sync::Mutex::new(0); }
```

_std Mutex type in async fn_

```rust
async fn f() { let _m: std::sync::Mutex<i32> = std::sync::Mutex::new(0); }
```

**Good (passes):**

_std Mutex in sync fn_

```rust
fn f() { let _m = std::sync::Mutex::new(0); }
```

_Mutex in async in test_

```rust
#[cfg(test)]
mod tests {
    async fn t() { let _m = std::sync::Mutex::new(0); }
}
```

### rust_native_escape_hatches

Require `unsafe fn from_native`, `into_native`, and `to_native` on public raw-pointer wrapper structs.

> Interop users need unsafe escape hatches to construct wrappers from native handles obtained elsewhere and to pass wrapped handles back over FFI.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_wrapper without any escape hatch_

```rust
pub struct Handle(*mut u8);
```

_from_native not unsafe_

```rust
pub struct Handle(*const u8);
impl Handle {
    pub fn from_native(raw: *const u8) -> Self { Self(raw) }
    pub fn into_native(self) -> *const u8 { self.0 }
    pub fn to_native(&self) -> *const u8 { self.0 }
}
```

_missing to_native_

```rust
pub struct Handle(*const u8);
impl Handle {
    pub unsafe fn from_native(raw: *const u8) -> Self { Self(raw) }
    pub fn into_native(self) -> *const u8 { self.0 }
}
```

**Good (passes):**

_wrapper with all escape hatches_

```rust
pub struct Handle(*const u8);
impl Handle {
    pub unsafe fn from_native(raw: *const u8) -> Self { Self(raw) }
    pub fn into_native(self) -> *const u8 { self.0 }
    pub fn to_native(&self) -> *const u8 { self.0 }
}
```

_private wrapper_

```rust
struct Handle(*const u8);
```

_pointer plus length is not a plain wrapper_

```rust
pub struct Slice {
    data: *const u8,
    len: usize,
}
```

_non-pointer field_

```rust
pub struct Id(u64);
```

_wrapper in test module_

```rust
#[cfg(test)]
mod tests {
    pub struct Handle(*const u8);
}
```

### rust_nested_smart_pointers

Flag directly nested heap pointers (`Arc<Box<T>>`, `Rc<Rc<T>>`, ...) plus `Arc<Vec<T>>`/`Arc<String>`.

> Each nesting layer is another sequential DRAM lookup on access — flatten to one allocation (Arc<[T]>, Arc<str>).

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_Arc<Box<T>> field_

```rust
struct S { a: Arc<Box<u32>> }
```

_Box<Arc<T>> parameter_

```rust
fn f(x: Box<Arc<u32>>) { drop(x); }
```

_Rc<Rc<T>> type alias_

```rust
type T = Rc<Rc<u32>>;
```

_Arc<Vec<T>> suggests Arc<[T]>_

```rust
struct S { d: Arc<Vec<u8>> }
```

_Arc<String> suggests Arc<str>_

```rust
struct S { n: Arc<String> }
```

**Good (passes):**

_Arc<Mutex<T>> is a single heap layer_

```rust
struct S { m: Arc<std::sync::Mutex<u32>> }
```

_Box<dyn Trait> is fine_

```rust
struct S { cb: Box<dyn Fn()> }
```

_Arc<[T]> and Arc<str> are the goal state_

```rust
fn f(b: Arc<[u8]>, s: Arc<str>) { drop(b); drop(s); }
```

_Box<Vec<T>> is owned by rust_box_vec, not this rule_

```rust
fn f() { let _x: Box<Vec<u32>> = Box::new(Vec::new()); }
```

_nested pointers in test module_

```rust
#[cfg(test)]
mod tests {
    struct S { a: Arc<Box<u32>> }
}
```

### rust_newtype_pub_field

Flag pub single-field structs exposing a pub primitive/`&str`/`String` field.

> A public weak-typed field bypasses any constructor-enforced invariant; make the field private and construct fallibly.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_pub tuple newtype with pub integer_

```rust
pub struct UserId(pub u64);
```

_pub tuple newtype with pub String_

```rust
pub struct Name(pub String);
```

_pub tuple newtype with pub str ref_

```rust
pub struct Label<'a>(pub &'a str);
```

_pub named newtype with pub field_

```rust
pub struct Port { pub value: u16 }
```

**Good (passes):**

_private field_

```rust
pub struct UserId(u64);
```

_two fields_

```rust
pub struct Point(pub f32, pub f32);
```

_non-pub struct_

```rust
struct Id(pub u32);
```

_non-primitive field_

```rust
pub struct Wrapper(pub Vec<u8>);
```

_newtype in test module_

```rust
#[cfg(test)]
mod tests {
    pub struct UserId(pub u64);
}
```

### rust_no_prelude

Ban `prelude` module declarations and `prelude.rs`/`prelude/mod.rs` files.

> Preludes invite glob imports that collide across crates and paper over bad module design.

|          |           |
| -------- | --------- |
| Severity | high      |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | no        |

**Bad (triggers violation):**

_pub prelude declaration_

```rust
pub mod prelude;
```

_private prelude declaration_

```rust
mod prelude;
```

_inline prelude module_

```rust
pub mod prelude {
    pub struct Token;
}
```

**Good (passes):**

_similarly named module_

```rust
mod prelude_ext;
```

_commented declaration_

```rust
// mod prelude;
```

_string mention_

```rust
fn f() { let s = "mod prelude;"; }
```

_regular module_

```rust
pub mod parsing;
```

### rust_non_exhaustive_on_public

Flag public enums without `#[non_exhaustive]` — prevents breaking changes when adding variants.

> Adding a variant to a public enum is a breaking change. #[non_exhaustive] lets you add variants without a major version bump.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | no       |
| Fixable  | no       |

**Bad (triggers violation):**

_public enum without non_exhaustive_

```rust
pub enum Color { Red, Green, Blue }
```

**Good (passes):**

_public enum with non_exhaustive_

```rust
#[non_exhaustive]
pub enum Color { Red, Green, Blue }
```

_private enum without non_exhaustive_

```rust
enum Color { Red, Green, Blue }
```

_pub(crate) enum without non_exhaustive_

```rust
pub(crate) enum Color { Red, Green, Blue }
```

_public enum in test_

```rust
#[cfg(test)]
mod tests {
    pub enum Color { Red, Green, Blue }
}
```

### rust_nonsend_across_await

Flag `Rc`/`RefCell` bindings in async code when an `.await` occurs later in the same block.

> !Send values held across an await point make the entire future !Send, breaking Tokio and other work-stealing runtimes.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_Rc::new held across await_

```rust
async fn f() { let rc = Rc::new(1); g().await; }
```

_RefCell::new held across await_

```rust
async fn f() { let cell = RefCell::new(1); g().await; }
```

_Rc::clone held across await_

```rust
async fn f(other: &Rc<u8>) { let rc = Rc::clone(other); g().await; }
```

_Rc in async block before await_

```rust
fn f() { let fut = async { let rc = Rc::new(1); g().await; }; }
```

**Good (passes):**

_await before the Rc binding_

```rust
async fn f() { g().await; let rc = Rc::new(1); }
```

_Arc is Send_

```rust
async fn f() { let arc = Arc::new(1); g().await; }
```

_Rc in sync fn_

```rust
fn f() { let rc = Rc::new(1); }
```

_await only inside a nested async block_

```rust
async fn f() { let rc = Rc::new(1); let fut = async { g().await; }; }
```

_Rc across await in test module_

```rust
#[cfg(test)]
mod tests {
    async fn f() { let rc = Rc::new(1); g().await; }
}
```

### rust_ok_or_eager

Flag `.ok_or()`/`.unwrap_or()` with eagerly evaluated arguments.

> ok_or() and unwrap_or() eagerly evaluate their argument even on the happy path. Use the \_else variant for expensive expressions.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | yes      |

**Bad (triggers violation):**

_ok_or with call_

```rust
fn f() { None::<i32>.ok_or(String::new()); }
```

_unwrap_or with call_

```rust
fn f() { None::<String>.unwrap_or(String::new()); }
```

**Good (passes):**

_ok_or with path_

```rust
fn f() { None::<i32>.ok_or(MyError::Static); }
```

_ok_or with struct literal_

```rust
fn f() { None::<i32>.ok_or(MyError { id: 1 }); }
```

_unwrap_or with literal_

```rust
fn f() { None::<i32>.unwrap_or(0); }
```

_unwrap_or with len_

```rust
fn f(v: &[u8]) { v.iter().position(|&b| b == 0).unwrap_or(v.len()); }
```

_ok_or in test module_

```rust
#[cfg(test)]
mod tests {
    fn t() { None::<i32>.ok_or(String::new()); }
}
```

### rust_owned_ref_param

Flag fn parameters typed `&String`, `&PathBuf`, `&Vec<T>`, `&OsString`.

> A shared reference to an owned container forces callers to materialize the owned type; borrowed forms accept more argument types for free.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | no       |
| Fixable  | no       |

**Bad (triggers violation):**

_&String parameter_

```rust
fn f(x: &String) {}
```

_&PathBuf parameter_

```rust
fn f(x: &PathBuf) {}
```

_&Vec parameter_

```rust
fn f(x: &Vec<u32>) {}
```

_fully qualified &OsString parameter_

```rust
fn f(x: &std::ffi::OsString) {}
```

_&String in impl method_

```rust
struct S;
impl S {
    fn f(&self, x: &String) {}
}
```

**Good (passes):**

_&str parameter_

```rust
fn f(x: &str) {}
```

_&mut String needs the owned type_

```rust
fn f(x: &mut String) {}
```

_owned String parameter_

```rust
fn f(x: String) {}
```

_slice parameter_

```rust
fn f(x: &[u32]) {}
```

_&String in test module_

```rust
#[cfg(test)]
mod tests {
    fn f(x: &String) {}
}
```

### rust_padding

Require configurable blank-line boundaries between functions and distinct statement groups.

> Consistent vertical separation makes functions, control flow, setup runs, and tail values easier to scan.

|                   |                                                                                              |
| ----------------- | -------------------------------------------------------------------------------------------- |
| Severity          | low                                                                                          |
| Type              | rust-ast                                                                                     |
| Enabled           | yes                                                                                          |
| Fixable           | yes                                                                                          |
| Param: boundaries | [String], default = ["functions", "control-flow", "let-runs", "returns", "tail-expressions"] |

**Bad (triggers violation):**

_return needs padding_

```rust
fn value() -> usize {
    let value = 1;
    return value;
}
```

_tail expression needs padding_

```rust
fn value() -> usize {
    let value = 1;
    value
}
```

_let run needs following padding_

```rust
fn run() {
    let one = 1;
    let two = 2;
    consume(one, two);
}
```

_multiline control expression needs padding before and after_

```rust
fn run(flag: bool) {
    prepare();
    if flag {
        work();
    }
    finish();
}
```

_let after multiline guard needs padding_

```rust
fn inspect(path: &Path) {
    if path.is_dir() {
        return;
    }
    let manifest = path.join("Cargo.toml");
}
```

_compact guard needs following padding_

```rust
fn inspect(path: &Path) {
    if path.is_dir() { return; }
    observe(path);
}
```

_compact consecutive guards need padding_

```rust
fn validate(one: bool, two: bool) {
    if one { return; }
    if two { return; }
}
```

_adjacent free functions need padding_

```rust
fn one() {}
fn two() {}
```

_adjacent methods need padding_

```rust
impl Value {
    fn one() {}
    fn two() {}
}
```

_adjacent trait methods need padding_

```rust
trait Value {
    fn one();
    fn two();
}
```

_directive stays attached to multiline control_

```rust
fn run(flag: bool) {
    let value = String::new();
    // #rw(rust_clone_in_loop) bounded control path
    if flag {
        consume(value.clone());
    }
}
```

_directive stays attached to tail expression_

```rust
fn value() -> usize {
    let value = 1;
    // #rw(rust_clone_in_loop) representative fixture
    value
}
```

_block directive stays attached to loop_

```rust
fn run(values: &[String]) {
    let mut copies = Vec::new();
    // #rw(block: rust_clone_in_loop) bounded fixture
    for value in values {
        copies.push(value.clone());
    }
}
```

**Good (passes):**

_return has padding_

```rust
fn value(flag: bool) -> usize {
    let value = 1;

    return value;
}
```

_first return is exempt_

```rust
fn value() -> usize {
    return 1;
}
```

_tail expression has padding_

```rust
fn value() -> usize {
    let value = 1;

    value
}
```

_let run followed by padding_

```rust
fn run() {
    let one = 1;
    let two = 2;

    consume(one, two);
}
```

_multiline control expression is padded_

```rust
fn run(flag: bool) {
    prepare();

    if flag {
        work();
    }

    finish();
}
```

_separated free functions_

```rust
fn one() {}

fn two() {}
```

_else-if remains one control-flow chain_

```rust
fn choose(one: bool, two: bool) {
    if one { work(); } else if two { work(); }
}
```

_first and last multiline controls need only interior padding_

```rust
fn run(flag: bool) {
    if flag {
        work();
    }

    loop {
        break;
    }
}
```

_single-expression closure is exempt_

```rust
fn run() {
    consume(|| { 1 });
}
```

### rust_panic

Ban `unimplemented!()`, `todo!()`, and message-less `panic!()` in library code.

> Detected programming bugs must panic with a message; todo!/unimplemented! mark unfinished code and message-less panics help nobody.

|          |          |
| -------- | -------- |
| Severity | high     |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_bare panic_

```rust
fn f() { panic!(); }
```

_empty panic message_

```rust
fn f() { panic!(""); }
```

_unimplemented in production_

```rust
fn f() { unimplemented!(); }
```

_todo in production_

```rust
fn f() { todo!(); }
```

**Good (passes):**

_panic with meaningful message (bug detection)_

```rust
fn f() { panic!("buffer len {} below header size", 3); }
```

_panic in test module_

```rust
#[cfg(test)]
mod tests {
    fn t() { panic!(); }
}
```

_no panic_

```rust
fn f() -> Result<(), String> { Ok(()) }
```

_expect is allowed_

```rust
fn f() { Some(1).expect("always Some"); }
```

### rust_panic_in_result_fn

Ban `panic!`, `.unwrap()`, `.expect()` in functions returning `Result`.

> A function returning Result promises fallible error handling. Panicking inside it breaks that contract.

|          |          |
| -------- | -------- |
| Severity | high     |
| Type     | rust-ast |
| Enabled  | no       |
| Fixable  | no       |

**Bad (triggers violation):**

_panic in Result fn_

```rust
fn f() -> Result<(), String> { panic!("x"); }
```

_unwrap in Result fn_

```rust
fn f() -> Result<(), String> { Some(1).unwrap(); Ok(()) }
```

_expect in Result fn_

```rust
fn f() -> Result<(), String> { Some(1).expect("msg"); Ok(()) }
```

_todo in Result fn_

```rust
fn f() -> Result<(), String> { todo!(); }
```

**Good (passes):**

_panic in non-Result fn_

```rust
fn f() -> i32 { panic!("x"); }
```

_Result fn with Err_

```rust
fn f() -> Result<(), String> { Err("x".into()) }
```

_panic in Result fn in test module_

```rust
#[cfg(test)]
mod tests {
    fn f() -> Result<(), String> { panic!("x"); }
}
```

### rust_panic_message

Require a message on `unreachable!` and `debug_assert!*`.

> Panic messages must state what went wrong; a missing or empty message gives the developer nothing to act on.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_unreachable without message_

```rust
fn f() { unreachable!(); }
```

_debug_assert without message_

```rust
fn f(x: u8) { debug_assert!(x > 0); }
```

_debug_assert_eq without message_

```rust
fn f(a: u8, b: u8) { debug_assert_eq!(a, b); }
```

_debug_assert_ne without message_

```rust
fn f(a: u8, b: u8) { debug_assert_ne!(a, b); }
```

_unreachable with empty message_

```rust
fn f() { unreachable!(""); }
```

**Good (passes):**

_unreachable with descriptive message_

```rust
fn f(m: u8) { unreachable!("month {m} out of range after validation"); }
```

_unreachable with short message_

```rust
fn f() { unreachable!("parser state was initialized before input"); }
```

_debug_assert with message_

```rust
fn f(x: u8) { debug_assert!(x > 0, "x must be positive, got {x}"); }
```

_debug_assert_eq with message_

```rust
fn f(a: u8, b: u8) { debug_assert_eq!(a, b, "lengths must match"); }
```

_plain assert not covered_

```rust
fn f(x: u8) { assert!(x > 0); }
```

_unreachable in test module_

```rust
#[cfg(test)]
mod tests {
    fn t() { unreachable!(); }
}
```

### rust_param_clump

Find maximal parameter groups repeated across functions; full-workspace runs are authoritative.

> Parameters that repeatedly travel together usually represent one missing domain value object.

|                  |                  |
| ---------------- | ---------------- |
| Severity         | low              |
| Type             | rust-workspace   |
| Enabled          | yes              |
| Fixable          | no               |
| Param: min_clump | i64, default = 3 |
| Param: min_fns   | i64, default = 3 |

**Bad (triggers violation):**

_three parameters travel across three functions_

```rust
fn one(user: String, region: u32, locale: String) {}
fn two(user: String, region: u32, locale: String, extra: bool) {}
fn three(user: String, region: u32, locale: String) {}
```

_pass through strengthens evidence_

```rust
fn sink(user: String, region: u32, locale: String) {}
fn middle(user: String, region: u32, locale: String) { sink(user, region, locale); }
fn source(user: String, region: u32, locale: String) { middle(user, region, locale); }
```

**Good (passes):**

_trait impl methods are exempt_

```rust
trait Handle { fn one(&self, user: String, region: u32, locale: String); fn two(&self, user: String, region: u32, locale: String); fn three(&self, user: String, region: u32, locale: String); }
struct Handler; impl Handle for Handler { fn one(&self, user: String, region: u32, locale: String) {} fn two(&self, user: String, region: u32, locale: String) {} fn three(&self, user: String, region: u32, locale: String) {} }
```

_only two functions_

```rust
fn one(user: String, region: u32, locale: String) {}
fn two(user: String, region: u32, locale: String) {}
```

### rust_param_order_consistency

Flag related fns whose shared parameters appear in a different order.

> Shared parameter order should stay stable within a real API family. Rulewright approximates that relationship with a common final name segment and the same impl or free-function module, so enable this only where that naming convention identifies meaningful families.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_shared pair order flips_

```rust
fn create_user(tenant: u32, user: u32) {}
fn delete_user(user: u32, tenant: u32) {}
```

_flip with interleaved params_

```rust
fn create_account(user: u32, tenant: u32, extra: bool) {}
fn delete_account(flag: bool, tenant: u32, user: u32) {}
```

_flip across impl fns_

```rust
struct S;
impl S {
    fn create_user(&self, user: u32, tenant: u32) {}
    fn delete_user(&self, tenant: u32, user: u32) {}
}
```

**Good (passes):**

_unrelated free functions are not compared_

```rust
fn create(tenant: u32, user: u32) {}
fn delete(user: u32, tenant: u32) {}
```

_consistent order_

```rust
fn create(tenant: u32, user: u32) {}
fn delete(tenant: u32, user: u32) {}
```

_only one shared pair_

```rust
fn f(a: u32, b: String) {}
fn g(b: String, c: u64) {}
```

_same names different types_

```rust
fn f(id: u32, name: String) {}
fn g(name: u64, id: String) {}
```

_single-param fns_

```rust
fn f(x: u32) {}
fn g(x: u32) {}
```

_flip in test module_

```rust
#[cfg(test)]
mod tests {
    fn f(user: u32, tenant: u32) {}
    fn g(tenant: u32, user: u32) {}
}
```

### rust_println

Ban `println!`/`eprintln!`/`print!`/`eprint!` outside test code.

> Console printing bypasses structured logging. Use tracing or the output module for consistent, filterable output.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | no       |
| Fixable  | no       |

**Bad (triggers violation):**

_println in library_

```rust
fn f() { println!("hello"); }
```

_eprintln in library_

```rust
fn f() { eprintln!("error"); }
```

_print in library_

```rust
fn f() { print!("hello"); }
```

_eprint in library_

```rust
fn f() { eprint!("error"); }
```

**Good (passes):**

_println in test module_

```rust
#[cfg(test)]
mod tests {
    fn t() { println!("debug"); }
}
```

_no println_

```rust
fn f() { let x = 1; }
```

_println in string literal_

```rust
fn f() { let s = "println!(value)"; }
```

### rust_proc_macro_thin_shim

Require proc-macro entry points to be thin `impl_crate::name(arg.into()).into()` shims.

> Token-stream logic living behind #[proc_macro] cannot be unit- or snapshot-tested — it belongs in a separate impl crate.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_inline expansion logic_

```rust
#[proc_macro]
pub fn my_macro(input: TokenStream) -> TokenStream {
    let text = input.to_string();
    text.parse().unwrap_or_default()
}
```

_delegation without impl crate_

```rust
#[proc_macro]
pub fn my_macro(input: TokenStream) -> TokenStream {
    expand(input)
}
```

**Good (passes):**

_thin function-like shim_

```rust
#[proc_macro]
pub fn my_macro(input: TokenStream) -> TokenStream {
    my_macro_impl::my_macro(input.into()).into()
}
```

_thin attribute shim with two args_

```rust
#[proc_macro_attribute]
pub fn route(attr: TokenStream, item: TokenStream) -> TokenStream {
    route_impl::route(attr.into(), item.into()).into()
}
```

_thin derive shim_

```rust
#[proc_macro_derive(Model, attributes(model))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    model_impl::derive_model(input.into()).into()
}
```

_plain fn with any body_

```rust
fn helper(input: String) -> String { input }
```

### rust_pub_api_docs

Require doc comments on public items.

> Undocumented public items force users to read source code. Doc comments generate searchable API documentation.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | no       |
| Fixable  | no       |

**Bad (triggers violation):**

_undocumented pub fn_

```rust
pub fn foo() {}
```

_undocumented pub struct_

```rust
pub struct S;
```

_undocumented pub enum_

```rust
pub enum E { A }
```

_undocumented pub trait_

```rust
pub trait T {}
```

_undocumented pub type alias_

```rust
pub type A = u8;
```

_undocumented pub const_

```rust
pub const C: u8 = 1;
```

_undocumented pub static_

```rust
pub static S: u8 = 1;
```

**Good (passes):**

_documented pub fn_

```rust
/// Does something.
pub fn foo() {}
```

_private fn_

```rust
fn foo() {}
```

_pub(crate) fn_

```rust
pub(crate) fn foo() {}
```

_doc hidden pub fn_

```rust
#[doc(hidden)]
pub fn foo() {}
```

_documented pub struct_

```rust
/// A struct.
pub struct S;
```

_pub(crate) struct not fully public_

```rust
pub(crate) struct S;
```

_doc hidden pub struct_

```rust
#[doc(hidden)]
pub struct S;
```

_doc name-value attr pub struct_

```rust
#[doc = "x"]
pub struct S;
```

### rust_pub_api_foreign_types

Flag foreign crate types leaked through `pub` fn signatures, fields, and type aliases.

> External types in public APIs tie the crate's stability to third-party crates; prefer std or workspace types. The external set is the owning crate's non-workspace [dependencies]; when no owning manifest exists (e.g. the test harness), `assume_external` supplies it.

|                        |                        |
| ---------------------- | ---------------------- |
| Severity               | low                    |
| Type                   | rust-ast               |
| Enabled                | no                     |
| Fixable                | no                     |
| Param: allowed         | [String], default = [] |
| Param: assume_external | [String], default = [] |

**Bad (triggers violation):**

_pub fn parameter of external type_

```rust
pub fn f(x: tokio::sync::Mutex<u32>) {}
```

_pub fn returning external type_

```rust
pub fn f() -> serde_json::Value { make() }
```

_external type nested in generic_

```rust
pub fn f(x: Vec<wasm_bindgen::JsValue>) {}
```

_pub struct field of external type_

```rust
pub struct S { pub v: syn::Type }
```

_pub enum variant of external type_

```rust
pub enum E { A(gix::ObjectId) }
```

_pub type alias to external type_

```rust
pub type Alias = winnow::error::ErrMode<u32>;
```

_pub method with external parameter_

```rust
pub struct S;
impl S {
    pub fn f(&self, x: rayon::ThreadPool) {}
}
```

**Good (passes):**

_private fn with external type_

```rust
fn f(x: tokio::sync::Mutex<u32>) {}
```

_pub(crate) fn with external type_

```rust
pub(crate) fn f(x: syn::Type) {}
```

_std type passes_

```rust
pub fn f(x: std::sync::Mutex<u32>) {}
```

_workspace type passes_

```rust
pub fn f(x: vendor_types::RecordId) {}
```

_bare imported name passes_

```rust
pub fn f(x: Value) {}
```

_private field on pub struct passes_

```rust
pub struct S { v: syn::Type }
```

_feature-gated fn passes_

```rust
#[cfg(feature = "serde")]
pub fn f(x: serde_json::Value) {}
```

_feature-gated impl block passes_

```rust
pub struct S;
#[cfg(feature = "tokio")]
impl S {
    pub fn f(&self, x: tokio::net::TcpStream) {}
}
```

_feature-gated type alias passes_

```rust
#[cfg(any(feature = "json", test))]
pub type Alias = serde_json::Value;
```

_pub fn in test module passes_

```rust
#[cfg(test)]
mod tests {
    pub fn f(x: syn::Type) {}
}
```

### rust_pub_api_generic_nesting

Flag pub fn signatures, pub struct fields, and pub type aliases nesting one local generic instantiation inside another (e.g. `Service<Backend<Store>>`).

> Nested crate-local generics infect user code with type parameters and trait bounds users never asked for; flatten or alias the composition.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_two local generic levels in param_

```rust
pub fn serve(s: Service<Backend<Store>>) {}
```

_two local generic levels in return_

```rust
pub fn build() -> Service<Backend<Store>> { make() }
```

_two local generic levels in pub field_

```rust
pub struct App {
    pub svc: Service<Backend<Store>>,
}
```

_two local generic levels in pub type alias_

```rust
pub type Handle = Service<Backend<Store>>;
```

_crate-prefixed local generics_

```rust
pub fn serve(s: crate::Service<crate::Backend<Store>>) {}
```

_local nesting under std container_

```rust
pub fn all() -> Vec<Service<Backend<Store>>> { Vec::new() }
```

**Good (passes):**

_std nesting_

```rust
pub fn buf(v: Vec<Vec<u8>>) {}
```

_one local level_

```rust
pub fn serve(s: Service<Backend>) {}
```

_local generic with std argument_

```rust
pub fn serve(s: Service<Vec<u8>>) {}
```

_foreign generics_

```rust
pub fn spawn(t: tokio::task::JoinHandle<foo::Bar<Baz>>) {}
```

_private fn nesting_

```rust
fn serve(s: Service<Backend<Store>>) {}
```

_nesting in test module_

```rust
#[cfg(test)]
mod tests {
    pub fn serve(s: Service<Backend<Store>>) {}
}
```

### rust_pub_api_smart_pointers

Flag `Rc`/`Arc`/`Box`/`RefCell`/`Cell`/`Mutex`/`RwLock` as the outermost type of pub fn params, returns, and pub struct fields.

> Smart pointers in public APIs leak implementation details and infect downstream signatures; accept and return plain types.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_arc parameter_

```rust
pub fn process(data: Arc<Mutex<Shared>>) {}
```

_box return_

```rust
pub fn build() -> Box<Processed> { Box::new(Processed) }
```

_rc refcell parameter_

```rust
pub fn init(config: Rc<RefCell<Config>>) {}
```

_pub wrapper field_

```rust
pub struct Server {
    pub state: Arc<State>,
}
```

_pub method with wrapper return_

```rust
pub struct S;
impl S {
    pub fn shared(&self) -> Rc<Data> { self.data.clone() }
}
```

**Good (passes):**

_private fn with wrapper_

```rust
fn helper(data: Arc<Config>) {}
```

_plain reference api_

```rust
pub fn process(data: &Data) -> State { data.state() }
```

_box dyn left to dyn rule_

```rust
pub fn run(handler: Box<dyn Handler>) {}
```

_boxed slice_

```rust
pub fn take(buf: Box<[u8]>) {}
```

_boxed str_

```rust
pub fn name() -> Box<str> { String::new().into_boxed_str() }
```

_private wrapper field_

```rust
pub struct Server {
    state: Arc<State>,
}
```

_wrapper not outermost_

```rust
pub fn all(items: Vec<Arc<Config>>) {}
```

_wrapper in test module_

```rust
#[cfg(test)]
mod tests {
    pub fn process(data: Arc<Mutex<Shared>>) {}
}
```

### rust_pub_use_grouping

Require public re-exports from the same origin to be adjacent.

> Keeping each re-export origin contiguous makes public API inventories easier to scan.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | yes      |

**Bad (triggers violation):**

_origin repeats after another group_

```rust
pub use alpha::One;
pub use beta::Two;
pub use alpha::Three;
```

**Good (passes):**

_origins are adjacent_

```rust
pub use alpha::One;
pub use alpha::Two;
pub use beta::Three;
```

_origin group order is author chosen_

```rust
pub use zebra::One;
pub use alpha::Two;
```

_plain use separates blocks_

```rust
pub use alpha::One;
use beta::Two;
pub use alpha::Three;
```

### rust_pub_use_position

Require top-level public imports to follow plain imports in a separate block.

> Keeping imports and re-exports in distinct leading blocks makes module API boundaries visible.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | yes      |

**Bad (triggers violation):**

_public import before plain import_

```rust
pub use crate::api::Thing;
use std::fmt;
```

_blocks need a blank line_

```rust
use std::fmt;
pub use crate::api::Thing;
```

**Good (passes):**

_plain imports before separated public imports_

```rust
use std::fmt;
use std::io;

pub use crate::api::Thing;
pub use crate::api::Other;
```

_inline module is exempt_

```rust
mod inner {
    pub use crate::api::Thing;
    use std::fmt;
}
```

_only public imports_

```rust
pub use crate::api::Thing;
pub use crate::api::Other;
```

### rust_public_error_enum

Flag `pub enum` named `*Error`/`*ErrorKind` — expose a situation-specific error struct with a private kind enum instead.

> A public error enum exposes every failure mode as breaking API surface; a struct wrapping a private kind enum keeps internal failure modes non-breaking.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | no       |
| Fixable  | no       |

**Bad (triggers violation):**

_pub error enum_

```rust
pub enum ParseError { Eof, Syntax }
```

_pub error kind enum_

```rust
pub enum IoErrorKind { NotFound, Denied }
```

**Good (passes):**

_private kind enum_

```rust
enum ErrorKind { Io, Protocol }
```

_pub(crate) error enum_

```rust
pub(crate) enum ParseError { Eof }
```

_pub enum without error suffix_

```rust
pub enum Mode { Fast, Slow }
```

_pub error struct_

```rust
pub struct ParseError { line: usize }
```

_pub error enum in test module_

```rust
#[cfg(test)]
mod tests {
    pub enum ParseError { Eof }
}
```

### rust_range_over_rangebounds

Flag `pub` fn parameters typed `Range<T>` — accept `impl RangeBounds<T>` instead.

> Range<T> forces callers to supply half-open bounds; impl RangeBounds<T> also accepts `1..`, `..=n`, and `..`.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_pub fn taking Range_

```rust
pub fn f(r: Range<usize>) {}
```

_pub fn taking std::ops::Range_

```rust
pub fn f(r: std::ops::Range<usize>) {}
```

_pub method taking Range_

```rust
pub struct S;
impl S {
    pub fn f(&self, r: Range<u32>) {}
}
```

**Good (passes):**

_private fn taking Range_

```rust
fn f(r: Range<usize>) {}
```

_impl RangeBounds parameter_

```rust
pub fn f(r: impl std::ops::RangeBounds<usize>) {}
```

_Range behind a reference_

```rust
pub fn f(r: &Range<usize>) {}
```

_RangeInclusive is not flagged_

```rust
pub fn f(r: RangeInclusive<usize>) {}
```

_Range in test module_

```rust
#[cfg(test)]
mod tests {
    pub fn f(r: Range<usize>) {}
}
```

### rust_recursive_fn

Flag direct self-recursion (stack overflow risk, especially in WASM).

> Direct recursion risks stack overflow, especially in WASM with its fixed 1MB stack. Use iteration or trampolining.

|          |          |
| -------- | -------- |
| Severity | high     |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_direct recursion (bare call)_

```rust
fn factorial(n: u64) -> u64 { if n <= 1 { 1 } else { n * factorial(n - 1) } }
```

_Self:: recursion_

```rust
struct S;
impl S { fn go(&self) { Self::go(self); } }
```

**Good (passes):**

_no recursion_

```rust
fn add(a: u64, b: u64) -> u64 { a + b }
```

_recursion in test module_

```rust
#[cfg(test)]
mod tests {
    fn factorial(n: u64) -> u64 { if n <= 1 { 1 } else { n * factorial(n - 1) } }
}
```

_calling different function_

```rust
fn foo() { bar(); }
fn bar() {}
```

_constructor calling other type constructors_

```rust
struct S { v: Vec<u8> }
impl S { fn new() -> Self { Self { v: Vec::new() } } }
```

_Default impl calling other defaults_

```rust
struct S { v: Vec<u8> }
impl Default for S { fn default() -> Self { Self { v: Vec::default() } } }
```

_qualified trait delegation_

```rust
struct S; trait T { fn go(); } impl T for S { fn go() { <u8 as T>::go(); } }
```

### rust_redundant_field_names

Flag `Foo { x: x }` — use shorthand `Foo { x }` instead.

> Rust supports field init shorthand (Foo { x } instead of Foo { x: x }). The long form is needless noise.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | yes      |

**Bad (triggers violation):**

_redundant field name_

```rust
struct S { x: i32 }
fn f(x: i32) -> S { S { x: x } }
```

**Good (passes):**

_shorthand field init_

```rust
struct S { x: i32 }
fn f(x: i32) -> S { S { x } }
```

_different name and value_

```rust
struct S { x: i32 }
fn f(y: i32) -> S { S { x: y } }
```

_redundant in test_

```rust
#[cfg(test)]
mod tests {
    struct S { x: i32 }
    fn t(x: i32) -> S { S { x: x } }
}
```

### rust_rulewright_directives

Enforce file-wide #rw directives at top of file with a blank line separator.

> File-wide rulewright directives belong at the very top so they are immediately visible. A blank line after them visually separates configuration from code.

|          |           |
| -------- | --------- |
| Severity | low       |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | no        |

**Bad (triggers violation):**

_file directive missing blank line before code_

```rust
// #rw(file: rust_panic) startup binary
use std::io;
```

_file directive after imports_

```rust
use std::io;
// #rw(file: rust_panic) startup binary

fn main() {}
```

_file directive then doc comment without blank line fails_

```rust
// #rw(file: rust_panic) startup binary
//! Module docs.

use std::io;
```

_mergeable directives with same reason_

```rust
// #rw(file: rust_panic) startup binary
// #rw(file: rust_unwrap_in_lib) startup binary

use std::io;
```

**Good (passes):**

_file directive at top with blank line_

```rust
// #rw(file: rust_panic) startup binary

use std::io;
```

_multiple file directives with different reasons_

```rust
// #rw(file: rust_panic) startup binary
// #rw(file: rust_unwrap_in_lib) CLI error handling

use std::io;
```

_non-file directive anywhere is fine_

```rust
use std::io;
// #rw(rust_panic) reason
panic!("x");
```

_file with no directives_

```rust
use std::io;
fn main() {}
```

_file directive then blank then doc comment passes_

```rust
// #rw(file: rust_panic) startup binary

//! Module docs.

use std::io;
```

_merged directives on one line_

```rust
// #rw(file: rust_panic, rust_unwrap_in_lib) startup binary

use std::io;
```

### rust_sensitive_debug

Flag `#[derive(Debug)]` on structs with sensitive fields like `password`.

> Deriving Debug on types with passwords or tokens risks leaking secrets in logs and error messages.

|                       |                                                                                                                                                                |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Severity              | high                                                                                                                                                           |
| Type                  | rust-ast                                                                                                                                                       |
| Enabled               | yes                                                                                                                                                            |
| Fixable               | no                                                                                                                                                             |
| Param: markers        | [String], default = ["api_key", "authorization", "bearer", "credential", "credentials", "password", "passwd", "private_key", "secret", "signing_key", "token"] |
| Param: allowed_fields | [String], default = []                                                                                                                                         |

**Bad (triggers violation):**

_Debug on struct with password field_

```rust
#[derive(Debug)]
struct Creds { password: String }
```

_Debug on struct with token field_

```rust
#[derive(Debug)]
struct Auth { token: String }
```

_Debug on struct with api_key field_

```rust
#[derive(Debug)]
struct Config { api_key: String }
```

_Debug on struct with bearer field_

```rust
#[derive(Debug)]
struct Auth { bearer: String }
```

_marker at a field-name boundary_

```rust
#[derive(Debug)]
struct Auth { oauth_bearer: String }
```

**Good (passes):**

_Debug on struct without sensitive fields_

```rust
#[derive(Debug)]
struct User { name: String }
```

_no Debug with password field_

```rust
struct Creds { password: String }
```

_marker text inside an unrelated word_

```rust
#[derive(Debug)]
struct Parser { tokenizer: String }
```

_sensitive struct in test module_

```rust
#[cfg(test)]
mod tests {
    #[derive(Debug)]
    struct Creds { password: String }
}
```

### rust_similar_fns

Find exact and near duplicate function bodies; full-workspace runs are authoritative.

> Clone detection identifies behavior that should usually be shared behind one implementation.

|                        |                   |
| ---------------------- | ----------------- |
| Severity               | low               |
| Type                   | rust-workspace    |
| Enabled                | yes               |
| Fixable                | no                |
| Param: min_tokens      | i64, default = 40 |
| Param: jaccard_percent | i64, default = 85 |

**Bad (triggers violation):**

_exact function bodies_

```rust
fn one(a: i32) -> i32 { let b = a + 1; let c = b + 2; let d = c + 3; let e = d + 4; let f = e + 5; let g = f + 6; let h = g + 7; let i = h + 8; let j = i + 9; let k = j + 10; k }
fn two(a: i32) -> i32 { let b = a + 1; let c = b + 2; let d = c + 3; let e = d + 4; let f = e + 5; let g = f + 6; let h = g + 7; let i = h + 8; let j = i + 9; let k = j + 10; k }
```

_near function bodies_

```rust
fn one(a: i32) -> i32 { let b = a + 1; let c = b + 2; let d = c + 3; let e = d + 4; let f = e + 5; let g = f + 6; let h = g + 7; let i = h + 8; let j = i + 9; let k = j + 10; k }
fn two(a: i32) -> i32 { let b = a + 1; let c = b + 2; let d = c + 3; let e = d + 4; let f = e + 5; let g = f + 6; let h = g + 8; let i = h + 8; let j = i + 9; let k = j + 10; k }
```

**Good (passes):**

_short bodies are exempt_

```rust
fn one() -> i32 { 1 }
fn two() -> i32 { 1 }
```

### rust_similar_structs

Find exact, near, and containment duplicate named-field structs; full-workspace runs are authoritative.

> Structural duplication often signals a missing shared domain type; indexed candidate generation keeps the check scalable.

|                        |                   |
| ---------------------- | ----------------- |
| Severity               | low               |
| Type                   | rust-workspace    |
| Enabled                | yes               |
| Fixable                | no                |
| Param: min_fields      | i64, default = 4  |
| Param: jaccard_percent | i64, default = 80 |

**Bad (triggers violation):**

_exact twins_

```rust
struct One { a: u32, b: String, c: bool, d: f64 }
struct Two { d: f64, c: bool, b: String, a: u32 }
```

_near twins_

```rust
struct One { a: u32, b: String, c: bool, d: f64 }
struct Two { a: u32, b: String, c: bool, d: f64, e: usize }
```

_containment twins_

```rust
struct One { a: u32, b: String, c: bool, d: f64 }
struct Two { a: u32, b: String, c: bool, d: f64, e: usize, f: usize }
```

**Good (passes):**

_input twin is sanctioned_

```rust
struct Request { a: u32, b: String, c: bool, d: f64 }
struct RequestInput { a: u32, b: String, c: bool, d: f64 }
```

_generic arity differs_

```rust
struct One<T> { a: u32, b: String, c: bool, d: T }
struct Two<T, U> { a: u32, b: String, c: bool, d: T, marker: U }
```

_below threshold_

```rust
struct One { a: u32, b: String, c: bool }
struct Two { a: u32, b: String, c: bool, d: f64 }
```

### rust_single_item_path

Flag `pub use` re-exports that duplicate paths already public through a sibling `pub mod`.

> Items reachable through two public paths clutter the API and confuse navigation; make the module non-pub or drop the re-export.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_pub mod with pub use reexport_

```rust
pub mod db {
    pub struct Connection;
}
pub use db::Connection;
```

_self-prefixed reexport_

```rust
pub mod db;
pub use self::db::Connection;
```

_grouped reexport_

```rust
pub mod db;
mod other;
pub use {db::Connection, other::Helper};
```

_renamed reexport_

```rust
pub mod db;
pub use db::Connection as Conn;
```

_module reexported under new name_

```rust
pub mod db;
pub use db as database;
```

_nested module siblings_

```rust
pub mod outer {
    pub mod inner {
        pub struct A;
    }
    pub use inner::A;
}
```

**Good (passes):**

_pub crate mod with pub use_

```rust
pub(crate) mod db {
    pub struct Connection;
}
pub use db::Connection;
```

_pub mod with private use_

```rust
pub mod db {
    pub struct Connection;
}
use db::Connection;
```

_doc hidden module_

```rust
#[doc(hidden)]
pub mod internals;
pub use internals::Helper;
```

_underscore module_

```rust
pub mod _private;
pub use _private::Helper;
```

_reexport from private sibling_

```rust
mod db {
    pub struct Connection;
}
pub use db::Connection;
```

_reexport in test module_

```rust
#[cfg(test)]
mod tests {
    pub mod db {
        pub struct Connection;
    }
    pub use db::Connection;
}
```

### rust_sorted

Enforce ordering in contiguous regions marked with `#rw:sorted(asc)` or `#rw:sorted(desc)`.

> Sorted lists prevent merge conflicts and make entries easy to locate.

|          |           |
| -------- | --------- |
| Severity | low       |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | yes       |

**Bad (triggers violation):**

_unsorted ascending region_

```rust
// #rw:sorted(asc)
use gamma;
use alpha;
use beta;
```

**Good (passes):**

_ascending region_

```rust
// #rw:sorted(asc)
use alpha;
use beta;
use gamma;
```

_descending region_

```rust
// #rw:sorted(desc)
use gamma;
use beta;
use alpha;
```

_no marker_

```rust
use zebra;
use alpha;
```

### rust_static_mut

Ban `static mut` declarations — use `AtomicT`, `Mutex`, or `OnceLock`.

> static mut is unsound in multithreaded code and deprecated. Use AtomicT, Mutex, or OnceLock instead.

|          |           |
| -------- | --------- |
| Severity | high      |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | no        |

**Bad (triggers violation):**

_static mut declaration_

```rust
static mut X: i32 = 0;
```

**Good (passes):**

_static Mutex_

```rust
static X: Mutex<i32> = Mutex::new(0);
```

_comment with static mut_

```rust
// static mut X: i32 = 0;
```

_static mut in string literal_

```rust
let msg = "static mut is dangerous";
```

### rust_string_error

Reject `String` and `&str` as function error types.

> Structured error types preserve context, support source chains, and give callers stable inspection APIs.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | no       |
| Fixable  | no       |

**Bad (triggers violation):**

_owned string error_

```rust
fn parse() -> Result<u32, String> { Err(String::new()) }
```

_borrowed string error_

```rust
fn parse() -> Result<u32, &'static str> { Err("bad") }
```

_qualified result_

```rust
fn parse() -> std::result::Result<u32, String> { Err(String::new()) }
```

**Good (passes):**

_typed error_

```rust
fn parse() -> Result<u32, ParseError> { todo!() }
```

_string success_

```rust
fn parse() -> Result<String, ParseError> { todo!() }
```

_test helper_

```rust
#[cfg(test)] mod tests { fn parse() -> Result<u32, String> { Err(String::new()) } }
```

### rust_style

Enforce no trailing whitespace, no tabs, no CRLF line endings.

> Trailing whitespace, tabs, and CRLF endings cause noisy diffs and merge conflicts.

|          |           |
| -------- | --------- |
| Severity | low       |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | yes       |

**Bad (triggers violation):**

_trailing whitespace_

```rust
let x = 1;
let y = 2;
```

_tab character_

```rust
	let x = 1;
```

**Good (passes):**

_clean file_

```rust
fn main() {}
```

### rust_subtractive_feature_cfg

Flag `#[cfg(not(feature = "..."))]` on `pub` items — features must be additive.

> A feature that removes public surface when enabled breaks feature unification: any dependent enabling it silently changes the API for everyone else.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_not-feature on pub fn_

```rust
#[cfg(not(feature = "std"))]
pub fn fallback() {}
```

_not-feature on pub struct_

```rust
#[cfg(not(feature = "alloc"))]
pub struct Fallback;
```

_not-feature on pub impl fn_

```rust
struct S;
impl S {
    #[cfg(not(feature = "std"))]
    pub fn fallback(&self) {}
}
```

**Good (passes):**

_positive feature gate_

```rust
#[cfg(feature = "std")]
pub fn f() {}
```

_not-feature on private fn_

```rust
#[cfg(not(feature = "std"))]
fn fallback() {}
```

_not-feature on pub(crate) fn_

```rust
#[cfg(not(feature = "std"))]
pub(crate) fn fallback() {}
```

_cfg not test is not a feature_

```rust
#[cfg(not(target_arch = "wasm32"))]
pub fn f() {}
```

### rust_tautological_assert

Flag test asserts comparing a constant against a literal (or literal vs literal).

> Asserts that restate a definition pass by construction and add noise instead of verifying behavior; test a property the value must satisfy instead.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_const vs literal_

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn t() { assert_eq!(MAX_SIZE, 10); }
}
```

_literal vs const_

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn t() { assert_eq!(3, Config::DEFAULT_RETRIES); }
}
```

_const vs array literal_

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn t() { assert_eq!(CHECKPOINTS, [0, 90, 180, 270]); }
}
```

_literal vs literal_

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn t() { assert_eq!(2, 2); }
}
```

_assert_ne const vs literal_

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn t() { assert_ne!(LIMIT, 0); }
}
```

_test attr without cfg test module_

```rust
#[test]
fn t() { assert_eq!(VERSION, "1.0"); }
```

**Good (passes):**

_behavior vs literal_

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn t() { assert_eq!(add(1, 2), 3); }
}
```

_variable vs const_

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn t() { let x = grow(); assert_eq!(x, MAX_SIZE); }
}
```

_method call vs literal_

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn t() { assert_eq!(result.len(), 4); }
}
```

_production assert is out of scope_

```rust
fn f() { assert_eq!(MAX_SIZE, 10); }
```

### rust_thiserror_qualified

Require thiserror derives to use the qualified `thiserror::Error` path.

> Qualified derives expose their provenance and avoid a file-wide trait import used only by attributes.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | yes      |

**Bad (triggers violation):**

_single bare derive_

```rust
use thiserror::Error;

#[derive(Debug, Error)]
#[error("failed")]
struct Failure;
```

_multiple bare derives_

```rust
use thiserror::Error;

#[derive(Error)]
#[error("one")]
struct One;
#[derive(Error)]
#[error("two")]
struct Two;
```

_import retained for a type use_

```rust
use thiserror::Error;

#[derive(Error)]
#[error("one")]
struct One;
fn accepts(value: &dyn Error) {}
```

**Good (passes):**

_qualified derive_

```rust
#[derive(Debug, thiserror::Error)]
#[error("failed")]
struct Failure;
```

_unrelated bare Error derive_

```rust
#[derive(Error)]
struct Failure;
```

### rust_todo

Require TODO/FIXME/HACK/XXX to have parenthesized context.

> TODO without context (who, ticket, deadline) becomes permanent. Parenthesized context ensures accountability.

|          |           |
| -------- | --------- |
| Severity | low       |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | no        |

**Bad (triggers violation):**

_bare TODO_

```rust
// TODO: fix this later
```

_bare FIXME_

```rust
// FIXME this is broken
```

_bare HACK_

```rust
// HACK: workaround
```

_bare XXX_

```rust
// XXX
```

_inline TODO_

```rust
let x = 1; // TODO fix
```

**Good (passes):**

_tracked TODO_

```rust
// TODO(#123) fix this
```

_no comment_

```rust
let todo = 5;
```

_tracked FIXME_

```rust
// FIXME(perf regression in v2)
```

### rust_too_many_lines_in_file

Flag files exceeding threshold lines.

> Files over 1500 lines are a sign that the module has too many responsibilities and should be split.

|                      |                     |
| -------------------- | ------------------- |
| Severity             | medium              |
| Type                 | rust-line           |
| Enabled              | yes                 |
| Fixable              | no                  |
| Param: threshold     | i64, default = 1500 |
| Param: slack_percent | i64, default = 20   |

### rust_trait_logic_not_inherent

Flag substantial logic in impls of locally-defined traits when the type has no same-named inherent method.

> Essential functionality buried in trait impls forces users to import the trait to call it; implement inherently and forward from the trait.

|                  |                  |
| ---------------- | ---------------- |
| Severity         | low              |
| Type             | rust-ast         |
| Enabled          | yes              |
| Fixable          | no               |
| Param: threshold | i64, default = 3 |

**Bad (triggers violation):**

_logic in trait impl without inherent method_

```rust
trait Download { fn get(&self); }
struct Client;
impl Download for Client {
    fn get(&self) {
        let a = 1;
        let b = a + 1;
        let c = b + 1;
        let _ = c;
    }
}
```

**Good (passes):**

_forwarding trait impl_

```rust
trait Download { fn get(&self); }
struct Client;
impl Client {
    fn get(&self) {
        let a = 1;
        let b = a + 1;
        let c = b + 1;
        let _ = c;
    }
}
impl Download for Client {
    fn get(&self) { Self::get(self) }
}
```

_logic with same-named inherent method elsewhere_

```rust
trait Download { fn get(&self); }
struct Client;
impl Client {
    fn get(&self) {}
}
impl Download for Client {
    fn get(&self) {
        let a = 1;
        let b = a + 1;
        let c = b + 1;
        let _ = c;
    }
}
```

_foreign trait impl exempt_

```rust
struct Client;
impl std::fmt::Display for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let a = 1;
        let b = a + 1;
        let c = b + 1;
        write!(f, "{c}")
    }
}
```

_body at threshold_

```rust
trait Download { fn get(&self); }
struct Client;
impl Download for Client {
    fn get(&self) {
        let a = 1;
        let b = a + 1;
        let _ = b;
    }
}
```

_logic in test module_

```rust
#[cfg(test)]
mod tests {
    trait Download { fn get(&self); }
    struct Client;
    impl Download for Client {
        fn get(&self) {
            let a = 1;
            let b = a + 1;
            let c = b + 1;
            let _ = c;
        }
    }
}
```

### rust_transmute_in_safe_fn

Flag `transmute` inside a safe `pub` fn.

> A safe public signature promises soundness its transmuting body cannot guarantee — the prime unsoundness suspect.

|          |          |
| -------- | -------- |
| Severity | high     |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_transmute in safe pub fn_

```rust
pub fn f(x: u32) -> f32 { unsafe { std::mem::transmute(x) } }
```

_transmute in safe pub method_

```rust
struct S;
impl S {
    pub fn f(&self, x: u32) -> f32 { unsafe { std::mem::transmute(x) } }
}
```

**Good (passes):**

_transmute in pub unsafe fn_

```rust
pub unsafe fn f(x: u32) -> f32 { unsafe { std::mem::transmute(x) } }
```

_transmute in private fn_

```rust
fn f(x: u32) -> f32 { unsafe { std::mem::transmute(x) } }
```

_transmute in pub(crate) fn_

```rust
pub(crate) fn f(x: u32) -> f32 { unsafe { std::mem::transmute(x) } }
```

_transmute in test module_

```rust
#[cfg(test)]
mod tests {
    pub fn t(x: u32) -> f32 { unsafe { std::mem::transmute(x) } }
}
```

### rust_transmute_usage

Require `SAFETY` comment on `std::mem::transmute` calls.

> transmute reinterprets raw bytes and can cause UB if the types are incompatible. A SAFETY comment proves correctness.

|          |          |
| -------- | -------- |
| Severity | high     |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_bare transmute without safety comment_

```rust
unsafe fn f() { let _x: u32 = std::mem::transmute(1.0f32); }
```

**Good (passes):**

_transmute with safety comment_

```rust
unsafe fn f() {
    // SAFETY: f32 and u32 have the same size
    let _x: u32 = std::mem::transmute(1.0f32);
}
```

_transmute in test module_

```rust
#[cfg(test)]
mod tests {
    unsafe fn t() { let _x: u32 = std::mem::transmute(1.0f32); }
}
```

### rust_type_def_ordering

Flag `impl` blocks that appear before their type definition.

> Reading code top-down, you expect to see a type defined before its methods. Impl-before-struct breaks that flow.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_impl before struct_

```rust
impl Foo { fn a() {} }
struct Foo;
```

_impl before enum_

```rust
impl Color { fn name(&self) -> &str { "" } }
enum Color { Red, Blue }
```

**Good (passes):**

_struct before impl_

```rust
struct Foo;
impl Foo { fn a() {} }
```

_trait impl before struct_

```rust
impl std::fmt::Display for Foo { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { Ok(()) } }
struct Foo;
```

_impl for external type_

```rust
impl Foo { fn a() {} }
```

_enum before impl_

```rust
enum Color { Red, Blue }
impl Color { fn name(&self) -> &str { "" } }
```

_impl before struct in test module_

```rust
#[cfg(test)]
mod tests {
    impl Foo { fn a() {} }
    struct Foo;
}
```

### rust_unbalanced_crate_root

Flag `lib.rs` roots that are flat item dumps (too many pub items) or empty shells (no pub items over many pub modules).

> A crate root with dozens of loose public items or nothing but module declarations is hard to navigate; balance essential items in the root with semantic submodules.

|                                   |                   |
| --------------------------------- | ----------------- |
| Severity                          | low               |
| Type                              | rust-ast          |
| Enabled                           | yes               |
| Fixable                           | no                |
| Param: max_root_items             | i64, default = 40 |
| Param: min_modules_for_empty_root | i64, default = 8  |

**Bad (triggers violation):**

_empty root over many modules_

```rust
pub mod a;
pub mod b;
pub mod c;
pub mod d;
pub mod e;
pub mod f;
pub mod g;
pub mod h;
```

**Good (passes):**

_balanced root_

```rust
pub struct Client;
pub mod account;
pub mod network;
```

_empty root with few modules_

```rust
pub mod a;
pub mod b;
pub mod c;
pub mod d;
pub mod e;
pub mod f;
pub mod g;
```

_many private modules_

```rust
mod a;
mod b;
mod c;
mod d;
mod e;
mod f;
mod g;
mod h;
pub use a::Client;
```

_many modules with root item_

```rust
pub struct Client;
pub mod a;
pub mod b;
pub mod c;
pub mod d;
pub mod e;
pub mod f;
pub mod g;
pub mod h;
```

### rust_unchecked_indexing

Flag `container[expr]` indexing with non-literal indices.

> Indexing with a variable panics on out-of-bounds. Prefer `.get()` when failure is possible. When an established invariant makes indexing correct, place a `// BOUNDS:` comment directly above it that names the concrete check or relationship; boilerplate comments do not make the code safer.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_variable index_

```rust
fn f(v: Vec<i32>, i: usize) { let _ = v[i]; }
```

**Good (passes):**

_literal index_

```rust
fn f(v: Vec<i32>) { let _ = v[0]; }
```

_indexing in test module_

```rust
#[cfg(test)]
mod tests {
    fn f(v: Vec<i32>, i: usize) { let _ = v[i]; }
}
```

_with BOUNDS comment_

```rust
fn f(v: Vec<i32>, i: usize) {
    assert!(i < v.len());
    // BOUNDS: the assertion above establishes i < v.len().
    let _ = v[i];
}
```

### rust_unnecessary_collect

Flag `.collect().iter()` — remove the intermediate collection.

> Collecting into a Vec just to iterate it again wastes an allocation. Chain the iterators directly.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_collect then iter_

```rust
fn f() { (0..10).collect::<Vec<i32>>().iter().count(); }
```

_collect then into_iter_

```rust
fn f() { (0..10).collect::<Vec<i32>>().into_iter().count(); }
```

**Good (passes):**

_separate collect_

```rust
fn f() { let v: Vec<i32> = (0..10).collect(); v.iter().count(); }
```

_collect without iter_

```rust
fn f() { let v: Vec<i32> = (0..10).collect(); }
```

_collect iter in test module_

```rust
#[cfg(test)]
mod tests {
    fn f() { (0..10).collect::<Vec<i32>>().iter().count(); }
}
```

### rust_unsafe_comment

Require `// SAFETY:` comment on `unsafe` blocks.

> Every unsafe block must document why it is sound. Without a SAFETY comment, reviewers cannot verify correctness.

|          |          |
| -------- | -------- |
| Severity | high     |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_unsafe without safety comment_

```rust
fn f() { unsafe { std::ptr::null::<u8>().read() }; }
```

**Good (passes):**

_unsafe with safety comment_

```rust
fn f() {
    // SAFETY: pointer is valid and aligned
    unsafe { std::ptr::null::<u8>().read() };
}
```

_unsafe in test module_

```rust
#[cfg(test)]
mod tests {
    fn t() { unsafe { std::ptr::null::<u8>().read() }; }
}
```

_safety comment with blank line_

```rust
fn f() {
    // SAFETY: guaranteed valid

    unsafe { std::ptr::null::<u8>().read() };
}
```

### rust_unsafe_fn_safety_doc

Require a `# Safety` doc section or `// SAFETY:` comment on every `unsafe fn`.

> Callers cannot uphold contracts that are not written down, and clippy's missing_safety_doc only covers public functions.

|          |          |
| -------- | -------- |
| Severity | high     |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_unsafe fn without safety docs_

```rust
unsafe fn f(x: u32) -> u32 { x }
```

_private unsafe fn without safety docs_

```rust
pub(crate) unsafe fn f() {}
```

_unsafe method without safety docs_

```rust
struct S;
impl S {
    unsafe fn m(&self) {}
}
```

**Good (passes):**

_unsafe fn with Safety doc section_

```rust
/// Reads the value behind `p`.
///
/// # Safety
/// `p` must be valid and aligned.
unsafe fn f(p: *const u8) -> u8 { unsafe { *p } }
```

_unsafe fn with SAFETY comment_

```rust
// SAFETY: callers uphold the invariants documented on the module
unsafe fn f() {}
```

_safe fn needs nothing_

```rust
fn f(x: u32) -> u32 { x }
```

_extern block fns are exempt_

```rust
extern "C" {
    fn ffi(x: u32) -> u32;
}
```

_unsafe fn in test module_

```rust
#[cfg(test)]
mod tests {
    unsafe fn t() {}
}
```

### rust_unsafe_impl_send

Flag `unsafe impl Send`/`Sync` without a `// SAFETY:` comment, and any generic (blanket) form.

> Bypassing Send/Sync bounds is the canonical unsoundness footgun, and a blanket impl over all T cannot be proven safe.

|          |           |
| -------- | --------- |
| Severity | high      |
| Type     | rust-line |
| Enabled  | yes       |
| Fixable  | no        |

**Bad (triggers violation):**

_unsafe impl Send without safety comment_

```rust
struct Foo(*mut u8);
unsafe impl Send for Foo {}
```

_unsafe impl Sync without safety comment_

```rust
struct Foo(*mut u8);
unsafe impl Sync for Foo {}
```

_generic blanket impl flagged even with safety comment_

```rust
struct Wrapper<T>(T);
// SAFETY: trust me
unsafe impl<T> Send for Wrapper<T> {}
```

_generic blanket Sync with bounds_

```rust
struct Wrapper<T>(T);
unsafe impl<T: Copy> Sync for Wrapper<T> {}
```

**Good (passes):**

_unsafe impl Send with safety comment_

```rust
struct Foo(*mut u8);
// SAFETY: the pointer is owned and never shared across threads
unsafe impl Send for Foo {}
```

_other unsafe trait impl_

```rust
struct Chunk([u8; 8]);
// SAFETY: all-zero bit pattern is valid
unsafe impl Zeroable for Chunk {}
```

_mention in string literal_

```rust
let s = "unsafe impl Send for Foo";
```

_mention in comment_

```rust
// unsafe impl Send for Foo
```

### rust_unsafe_without_ub_surface

Flag `unsafe fn` with no raw-pointer surface and no unsafe operations in the body.

> `unsafe` may only mark undefined-behavior risk, not general danger — a fn without UB surface trains callers to ignore the keyword.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_unsafe as danger marker_

```rust
unsafe fn delete_everything(name: &str) { let _ = name; }
```

_unsafe method without UB surface_

```rust
struct S;
impl S {
    pub unsafe fn clear(&mut self) {}
}
```

**Good (passes):**

_raw pointer parameter_

```rust
unsafe fn read_at(p: *const u8) -> u8 { *p }
```

_NonNull parameter_

```rust
unsafe fn touch(p: std::ptr::NonNull<u8>) { let _ = p; }
```

_raw pointer return type_

```rust
unsafe fn alloc_raw() -> *mut u8 { std::ptr::null_mut() }
```

_unsafe block in body_

```rust
unsafe fn call() { unsafe { std::ptr::null::<u8>().read(); } }
```

_extern abi is exempt_

```rust
unsafe extern "C" fn callback() {}
```

_no_mangle is exempt_

```rust
#[no_mangle]
pub unsafe fn hook() {}
```

_safe fn_

```rust
fn f(x: u32) -> u32 { x }
```

### rust_unwrap_in_lib

Ban `.unwrap()` in library code.

> unwrap() in library code panics the caller with no context. Return Result or use expect() with a message.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | no       |
| Fixable  | no       |

**Bad (triggers violation):**

_unwrap in library_

```rust
fn f() { Some(1).unwrap(); }
```

**Good (passes):**

_expect passes_

```rust
fn f() { Some(1).expect("reason"); }
```

_unwrap in test module_

```rust
#[cfg(test)]
mod tests {
    fn t() { Some(1).unwrap(); }
}
```

### rust_vec_init_then_push

Flag `Vec::new()` immediately followed by `.push()` calls (use `vec![]` or `with_capacity`).

> Vec::new() followed by push() calls can be replaced with vec![...] or Vec::with_capacity for clarity and performance.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_Vec::new then push_

```rust
fn f() { let mut v = Vec::new(); v.push(1); v.push(2); }
```

**Good (passes):**

_vec! macro is fine_

```rust
fn f() { let v = vec![1, 2]; }
```

_Vec::new with_capacity is fine_

```rust
fn f() { let mut v = Vec::with_capacity(10); v.push(1); }
```

_Vec::new then push in test_

```rust
#[cfg(test)]
mod tests {
    fn t() { let mut v = Vec::new(); v.push(1); }
}
```

_Vec::new with logic between_

```rust
fn f() { let mut v = Vec::new(); let x = 1; if x > 0 { v.push(x); } }
```

_annotated Vec binding is outside the simple-pattern rule_

```rust
fn f() { let mut v: Vec<u32> = Vec::new(); v.push(1); }
```

### rust_vec_string_field

Flag non-pub struct fields typed `Vec<String>` or `Vec<Vec<T>>`.

> Immutable-after-construction sequences stored as Vec<Box<str>>/Vec<Box<[T]>> drop the capacity word and imply shrink-to-fit.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | no       |
| Fixable  | no       |

**Bad (triggers violation):**

_private Vec<String> field_

```rust
struct S { names: Vec<String> }
```

_private Vec<Vec<T>> field_

```rust
struct S { grid: Vec<Vec<u8>> }
```

_pub(crate) field is not fully public_

```rust
pub struct S { pub(crate) names: Vec<String> }
```

_private tuple-struct field_

```rust
struct S(Vec<String>);
```

**Good (passes):**

_fully public field is user-visible_

```rust
pub struct S { pub names: Vec<String> }
```

_already boxed str elements_

```rust
struct S { names: Vec<Box<str>> }
```

_primitive element vector_

```rust
struct S { ids: Vec<u32> }
```

_Vec<String> field in test module_

```rust
#[cfg(test)]
mod tests {
    struct S { names: Vec<String> }
}
```

_record enum variant is outside the struct-only rule_

```rust
enum E { Names { values: Vec<String> } }
```

_tuple enum variant is outside the struct-only rule_

```rust
enum E { Names(Vec<String>) }
```

### rust_weasel_words

Flag type definitions whose name contains a weasel word like `Manager`, `Service`, or `Factory`.

> Weasel words add no information: `Bookings` beats `BookingManager`, and Rust's name for a factory is `Builder`.

|              |                                                       |
| ------------ | ----------------------------------------------------- |
| Severity     | medium                                                |
| Type         | rust-ast                                              |
| Enabled      | yes                                                   |
| Fixable      | no                                                    |
| Param: words | [String], default = ["Factory", "Manager", "Service"] |

**Bad (triggers violation):**

_Manager type_

```rust
struct BookingManager;
```

_Service trait_

```rust
trait BookingService {}
```

_Factory enum_

```rust
enum WidgetFactory { A }
```

_Service type alias_

```rust
type AccountService = ();
```

_weasel word in the middle_

```rust
struct ManagerConfig;
```

**Good (passes):**

_word only as partial segment_

```rust
struct Managed;
```

_descriptive name_

```rust
struct BookingDispatcher;
```

_use of a weasel type is not a definition_

```rust
use remote::BookingManager;
```

_weasel-named fn is not a type definition_

```rust
fn manager() {}
```

_weasel type in test module_

```rust
#[cfg(test)]
mod tests {
    struct FakeManager;
}
```

### rust_where_clauses

Require type-parameter trait bounds to use where clauses.

> Where clauses keep function and type declarations readable as constraints grow.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_inline function bound_

```rust
fn render<T: std::fmt::Display>(value: T) {}
```

_inline struct bound_

```rust
struct Wrapper<T: Clone> { value: T }
```

**Good (passes):**

_function where clause_

```rust
fn render<T>(value: T) where T: std::fmt::Display {}
```

_lifetime bounds are exempt_

```rust
struct Borrowed<'a: 'static> { value: &'a str }
```

_const generics are exempt_

```rust
struct Buffer<const N: usize> { bytes: [u8; N] }
```

_default type params are exempt_

```rust
struct Wrapper<T: Clone = String> { value: T }
```

### rust_wildcard_imports

Ban `use foo::*` outside tests and preludes.

> Wildcard imports make it unclear where names come from and cause surprising breakage when upstream adds new items.

|          |          |
| -------- | -------- |
| Severity | medium   |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_wildcard import_

```rust
use std::collections::*;
```

_private third party glob_

```rust
use some_crate::types::*;
```

_deep external glob_

```rust
use external_crate::deep::module::*;
```

**Good (passes):**

_explicit import_

```rust
use std::collections::HashMap;
```

_prelude allowed_

```rust
use my_crate::prelude::*;
```

_super star allowed_

```rust
use super::*;
```

_super nested allowed_

```rust
use super::types::*;
```

_pub use reexport allowed_

```rust
pub use my_module::*;
```

_pub crate reexport allowed_

```rust
pub(crate) use my_module::*;
```

_enum glob in fn allowed_

```rust
fn f() { use MyEnum::*; }
```

_enum glob at module level allowed_

```rust
use ColType::*;
```

_crate internal types glob allowed_

```rust
use crate::types::*;
```

_wildcard in test module_

```rust
#[cfg(test)]
mod tests {
    use std::collections::*;
}
```

_nested prelude allowed_

```rust
use some_crate::module::prelude::*;
```

### rust_yoda_conditions

Flag reversed comparisons like `0 == x` — prefer `x == 0`.

> In Rust there is no accidental assignment in conditions, so the C-style 0 == x guard is unnecessary and harder to read.

|          |          |
| -------- | -------- |
| Severity | low      |
| Type     | rust-ast |
| Enabled  | yes      |
| Fixable  | no       |

**Bad (triggers violation):**

_literal on left eq_

```rust
fn f(x: i32) { if 0 == x {} }
```

_literal on left ne_

```rust
fn f(y: i32) { if 1 != y {} }
```

**Good (passes):**

_literal on right_

```rust
fn f(x: i32) { if x == 0 {} }
```

_both literals_

```rust
fn f() { if "a" == "b" {} }
```

_non-comparison_

```rust
fn f(x: i32) { let _ = 1 + x; }
```

_yoda in test module_

```rust
#[cfg(test)]
mod tests {
  fn t(x: i32) { if 0 == x {} }
}
```

### toml_ambiguous_unicode

Ban Unicode characters in TOML that are visually confusable with ASCII.

> Confusable punctuation and homoglyphs make manifest comments and values easy to misread.

|          |      |
| -------- | ---- |
| Severity | high |
| Type     | toml |
| Enabled  | yes  |
| Fixable  | no   |

**Bad (triggers violation):**

_ambiguous punctuation in comment_

```rust
# range 1–5
value = 1

```

_homoglyph in string_

```rust
name = "pаssword"

```

**Good (passes):**

_normal ASCII_

```rust
# range 1-5
value = 1

```

_unambiguous non-ASCII_

```rust
# résumé
value = 1

```

### toml_bidirectional_unicode

Ban Unicode bidi control characters in TOML.

> Bidi controls can reorder displayed manifests and comments to conceal dependency or configuration changes.

|          |      |
| -------- | ---- |
| Severity | high |
| Type     | toml |
| Enabled  | yes  |
| Fixable  | no   |

**Bad (triggers violation):**

_bidi control in string_

```rust
name = "safe‮txt"

```

_bidi control in comment_

```rust
# safe⁦txt
name = "plain"

```

**Good (passes):**

_ordinary TOML_

```rust
name = "plain"

```

### toml_cargo_edition

Require the workspace and non-inheriting members to target at least the configured Rust edition, with the matching virtual-workspace resolver.

> Virtual workspaces do not infer the resolver from workspace.package.edition, so resolver 3 must be explicit for edition 2024.

|                    |                     |
| ------------------ | ------------------- |
| Severity           | medium              |
| Type               | toml                |
| Enabled            | yes                 |
| Fixable            | no                  |
| Param: min_edition | i64, default = 2024 |

**Bad (triggers violation):**

_workspace on current edition_

```rust
[workspace]
members = []

[workspace.package]
edition = "2024"

```

_workspace on old edition_

```rust
[workspace]
resolver = "2"
members = []

[workspace.package]
edition = "2021"

```

_member omits edition_

```rust
[package]
name = "foo"

```

_member on old edition_

```rust
[package]
name = "foo"
edition = "2021"

```

**Good (passes):**

_redundant resolver on current edition_

```rust
[workspace]
resolver = "3"
members = []

[workspace.package]
edition = "2024"

```

_member inherits edition_

```rust
[package]
name = "foo"
edition.workspace = true

```

### toml_cargo_feature_names

Flag Cargo feature names with use-/with- prefixes or -support suffixes.

> Feature names should describe the capability itself; placeholder affixes add noise without meaning (C-FEATURE).

|                |                        |
| -------------- | ---------------------- |
| Severity       | low                    |
| Type           | toml                   |
| Enabled        | yes                    |
| Fixable        | no                     |
| Param: allowed | [String], default = [] |

**Bad (triggers violation):**

_use- prefix_

```rust
[features]
use-serde = ["dep:serde"]

```

_with\_ prefix_

```rust
[features]
with_tokio = ["dep:tokio"]

```

_-support suffix_

```rust
[features]
serde-support = ["dep:serde"]

```

_\_support suffix_

```rust
[features]
serde_support = ["dep:serde"]

```

**Good (passes):**

_capability-named features_

```rust
[features]
default = []
std = []
serde = ["dep:serde"]

```

_no features table_

```rust
[package]
name = "foo"

```

### toml_cargo_feature_no_std

Ban subtractive no-std Cargo features; provide an additive std feature instead.

> Features must be additive so any combination compiles; a no-std feature that removes functionality breaks feature unification.

|          |        |
| -------- | ------ |
| Severity | medium |
| Type     | toml   |
| Enabled  | yes    |
| Fixable  | no     |

**Bad (triggers violation):**

_no-std feature_

```rust
[features]
no-std = []

```

_no_std feature_

```rust
[features]
no_std = []

```

_nostd feature_

```rust
[features]
nostd = []

```

**Good (passes):**

_additive std feature_

```rust
[features]
default = ["std"]
std = []

```

_no features table_

```rust
[package]
name = "foo"

```

### toml_cargo_msrv

Require the workspace to declare rust-version and members to inherit it instead of overriding it.

> A declared MSRV makes the supported-compiler contract explicit, and per-crate overrides silently fragment it.

|          |      |
| -------- | ---- |
| Severity | low  |
| Type     | toml |
| Enabled  | yes  |
| Fixable  | no   |

**Bad (triggers violation):**

_workspace without MSRV_

```rust
[workspace]
members = []

[workspace.package]
edition = "2024"

```

_member omits MSRV inheritance_

```rust
[package]
name = "foo"

```

_member overrides MSRV_

```rust
[package]
name = "foo"
rust-version = "1.85"

```

**Good (passes):**

_workspace declares MSRV_

```rust
[workspace]
members = []

[workspace.package]
edition = "2024"
rust-version = "1.85"

```

_member inherits MSRV_

```rust
[package]
name = "foo"
rust-version.workspace = true

```

### toml_cargo_unused_deps

Flag workspace-member dependencies that are never referenced by any Rust compile target.

> Unused dependencies increase build time and supply-chain surface. Full-workspace runs are authoritative because references are aggregated across src, tests, benches, examples, and build.rs.

|                    |                        |
| ------------------ | ---------------------- |
| Severity           | medium                 |
| Type               | workspace              |
| Enabled            | yes                    |
| Fixable            | no                     |
| Param: always_used | [String], default = [] |

**Bad (triggers violation):**

_unused dependency_

```rust
fn main() {}
```

**Good (passes):**

_path-referenced dependency_

```rust
fn encode(value: serde::Value) {}
```

_derive-only dependency_

```rust
#[derive(serde::Serialize)] struct Value;
```

### toml_cargo_workspace_dep_features

Flag [workspace.dependencies] entries that enable features outside the allowlist.

> Features are additive and belong to the consuming crate; the workspace table should only pin versions so members do not inherit unwanted features.

|                         |                                       |
| ----------------------- | ------------------------------------- |
| Severity                | low                                   |
| Type                    | toml                                  |
| Enabled                 | yes                                   |
| Fixable                 | no                                    |
| Param: allowed_features | [String], default = ["derive", "std"] |

**Bad (triggers violation):**

_feature outside allowlist_

```rust
[workspace.dependencies]
tsify = { version = "0.5", features = ["js"] }

```

**Good (passes):**

_allowlisted features_

```rust
[workspace.dependencies]
serde = { version = "1", features = ["derive", "std"] }

```

_no features_

```rust
[workspace.dependencies]
serde = "1"
thiserror = { version = "2", default-features = false }

```

### toml_cargo_workspace_lints

Require the workspace to enable the standard rust/clippy lint set and members to inherit it via `[lints] workspace = true`.

> Static verification only catches issues when every crate compiles under the same vetted workspace lint tables.

|                        |                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Severity               | medium                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Type                   | toml                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Enabled                | yes                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Fixable                | no                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Param: required_rust   | [String], default = ["ambiguous_negative_literals", "missing_debug_implementations", "redundant_imports", "redundant_lifetimes", "trivial_numeric_casts", "unsafe_op_in_unsafe_fn", "unused_lifetimes"]                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Param: required_clippy | [String], default = ["cargo", "complexity", "correctness", "pedantic", "perf", "style", "suspicious", "allow_attributes_without_reason", "as_pointer_underscore", "assertions_on_result_states", "clone_on_ref_ptr", "deref_by_slicing", "disallowed_script_idents", "empty_drop", "empty_enum_variants_with_brackets", "empty_structs_with_brackets", "fn_to_numeric_cast_any", "if_then_some_else_none", "implicit_clone", "map_err_ignore", "redundant_type_annotations", "renamed_function_params", "semicolon_outside_block", "undocumented_unsafe_blocks", "unnecessary_safety_comment", "unnecessary_safety_doc", "unneeded_field_pattern", "unused_result_ok"] |

**Bad (triggers violation):**

_workspace missing required lints_

```rust
[workspace]
members = []

```

_workspace lint downgraded to allow_

```rust
[workspace.lints.rust]
unsafe_op_in_unsafe_fn = "allow"

```

_member without lint inheritance_

```rust
[package]
name = "foo"

```

**Good (passes):**

_workspace with full lint set_

```rust
[workspace.lints.rust]
ambiguous_negative_literals = "warn"
missing_debug_implementations = "warn"
redundant_imports = "warn"
redundant_lifetimes = "warn"
trivial_numeric_casts = "warn"
unsafe_op_in_unsafe_fn = "warn"
unused_lifetimes = "warn"

[workspace.lints.clippy]
cargo = { level = "warn", priority = -1 }
complexity = { level = "warn", priority = -1 }
correctness = { level = "deny", priority = -1 }
pedantic = { level = "warn", priority = -1 }
perf = { level = "warn", priority = -1 }
style = { level = "warn", priority = -1 }
suspicious = { level = "warn", priority = -1 }
allow_attributes_without_reason = "warn"
as_pointer_underscore = "warn"
assertions_on_result_states = "warn"
clone_on_ref_ptr = "warn"
deref_by_slicing = "warn"
disallowed_script_idents = "warn"
empty_drop = "warn"
empty_enum_variants_with_brackets = "warn"
empty_structs_with_brackets = "warn"
fn_to_numeric_cast_any = "warn"
if_then_some_else_none = "warn"
implicit_clone = "warn"
map_err_ignore = "warn"
redundant_type_annotations = "warn"
renamed_function_params = "warn"
semicolon_outside_block = "warn"
undocumented_unsafe_blocks = "warn"
unnecessary_safety_comment = "warn"
unnecessary_safety_doc = "warn"
unneeded_field_pattern = "warn"
unused_result_ok = "warn"

```

_member inherits workspace lints_

```rust
[package]
name = "foo"

[lints]
workspace = true

```

### toml_validity

Reject TOML syntax errors and semantic conflicts such as duplicate keys.

> Taplo validation catches malformed documents before language-specific consumers produce inconsistent diagnostics.

|          |      |
| -------- | ---- |
| Severity | high |
| Type     | toml |
| Enabled  | yes  |
| Fixable  | no   |

**Bad (triggers violation):**

_syntax error_

```rust
name =

```

_duplicate key_

```rust
name = "first"
name = "second"

```

**Good (passes):**

_valid TOML_

```rust
name = "example"
[table]
value = 1

```
