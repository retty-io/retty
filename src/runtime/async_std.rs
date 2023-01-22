use std::{future::Future, pin::Pin};

use super::Runtime;

/// A runtime for async-std
#[derive(Debug)]
pub struct AsyncStdRuntime;

impl Runtime for AsyncStdRuntime {
    fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send>>) {
        async_std::task::spawn(future);
    }
}

/// A wrapper for async-std sync mod
pub mod sync {
    pub use async_std::sync::Mutex;
}

/// A wrapper for async-std net mod
pub mod net {
    pub use async_std::net::{TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
    /// A OwnedReadHalf wrapper for async-std TcpStream
    pub type OwnedReadHalf = async_std::net::TcpStream;
    /// A OwnedWriteHalf wrapper for async-std TcpStream
    pub type OwnedWriteHalf = async_std::net::TcpStream;
}

/// A wrapper for async-std io mod
pub mod io {
    pub use async_std::io::{Read, Write};
    pub use futures_lite::io::{AsyncReadExt, AsyncWriteExt};
}

/// A wrapper for async-std mpsc mod
pub mod mpsc {
    pub use async_std::channel::{bounded, Receiver, Sender};
}

/// A wrapper for async-std sleep
pub use async_std::task::sleep;
