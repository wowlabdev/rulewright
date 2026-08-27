//! Rust safety and security rules.

mod abs_home_path;
mod ambient_syscall;
mod build_rs_external_tool;
mod hardcoded_url;
mod mutex_in_async;
mod sensitive_debug;
mod support;
mod unsafe_comment;
mod unsafe_fn_safety_doc;
mod unsafe_impl_send;
mod unsafe_without_ub_surface;

pub(crate) use sensitive_debug::has_sensitive_fields;
