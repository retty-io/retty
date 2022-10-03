#![warn(rust_2018_idioms)]
#![allow(dead_code)]

pub mod bootstrap;
pub mod channel;
pub mod codec;
pub mod error;
pub mod runtime;
pub mod transport;

pub struct Message {
    pub transport: transport::TransportContext,
    pub body: Box<dyn std::any::Any + Send + Sync>,
}
