#![warn(rust_2018_idioms)]
#![allow(dead_code)]

use std::any::Any;

pub mod bootstrap;
pub mod channel;
pub mod codec;
pub mod error;
pub mod transport;

use crate::transport::TransportContext;

pub struct Message {
    pub transport: TransportContext,
    pub body: Box<dyn Any + Send + Sync>,
}
