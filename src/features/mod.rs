//! Built-in feature modules. Adding a feature means adding its module and one
//! line to [`BUILTINS`].

pub mod point;
pub mod system;

use crate::registry::RegisterFn;

/// Order matters only for display ordering ties; the first registered tool is
/// the default tool.
pub const BUILTINS: &[RegisterFn] = &[point::register, system::register];
