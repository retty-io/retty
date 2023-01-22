//! Retty is an asynchronous Rust networking framework that makes it easy to build protocols, application clients/servers.
//!
//! It's like [Netty](https://netty.io) or [Wangle](https://github.com/facebook/wangle), but in Rust.
//!

#![warn(rust_2018_idioms)]
#![allow(dead_code)]
#![warn(missing_docs)]

pub mod bootstrap;
pub mod channel;
pub mod codec;
pub mod error;
pub mod runtime;
pub mod transport;
