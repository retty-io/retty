#![warn(rust_2018_idioms)]
#![allow(dead_code)]

use std::any::Any;

pub mod bootstrap;
pub mod channel;
pub mod codec;
pub mod error;
pub mod runtime;
pub mod transport;

#[cfg(feature = "runtime-async-std")]
pub use crate::runtime::AsyncStdRuntime;

#[cfg(feature = "runtime-tokio")]
pub use crate::runtime::TokioRuntime;

pub use crate::runtime::{default_runtime, Runtime};
pub use crate::transport::TransportContext;

pub struct Message {
    pub transport: TransportContext,
    pub body: Box<dyn Any + Send + Sync>,
}
