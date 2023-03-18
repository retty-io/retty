//! Common utils

pub(crate) mod linked_list;
pub(crate) mod slab;
pub(crate) mod thread_id;

mod rand;
pub use rand::thread_rng_n;

#[cfg(feature = "utils")]
mod bind_to_cpu_set;
#[cfg(feature = "utils")]
pub use bind_to_cpu_set::{bind_to_cpu_set, BindError};
