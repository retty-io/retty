#![warn(rust_2018_idioms)]
#![allow(dead_code)]

use std::any::Any;

pub mod bootstrap;
pub mod channel;
pub mod codec;
pub mod error;
pub mod runtime;
pub mod transport;

type Message = dyn Any + Send + Sync;
