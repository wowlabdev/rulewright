//! Rust correctness rules.

mod ambiguous_unicode;
mod assert_side_effects;
mod bidirectional_unicode;
mod catch_unwind;
mod deep_exit;
mod drop_panic;
mod dup_expressions;
mod exotic_numeric_api;
mod floating_point_eq;
mod global_state;
mod infallible_from_weak;
mod lossy_cast;
mod mem_forget;
mod newtype_pub_field;
mod panic;
mod panic_in_result_fn;
mod panic_message;
mod static_mut;
mod support;
mod transmute_in_safe_fn;
mod transmute_usage;
mod unchecked_indexing;
