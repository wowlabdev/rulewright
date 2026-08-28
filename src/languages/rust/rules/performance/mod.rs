//! Rust performance rules.

mod alloc_in_loop;
mod async_loop_no_yield;
mod box_leak;
mod box_vec;
mod busy_wait;
mod collection_new_in_loop;
mod default_hasher;
mod from_instead_of_as;
mod large_async_local;
mod missing_capacity;
mod nested_smart_pointers;
mod ok_or_eager;
mod support;
mod unnecessary_collect;
mod vec_init_then_push;
mod vec_string_field;
