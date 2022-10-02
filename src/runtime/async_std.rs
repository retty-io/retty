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

pub mod sync {
    pub use async_std::sync::Mutex;
}

pub mod net {
    pub use async_std::net::{TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
    pub type OwnedReadHalf = async_std::net::TcpStream;
    pub type OwnedWriteHalf = async_std::net::TcpStream;
}

pub mod io {
    pub use async_std::io::{Read, Write};
    pub use futures_lite::io::{AsyncReadExt, AsyncWriteExt};
}

pub mod mpsc {
    pub use async_std::channel::{bounded, Receiver, Sender};
}
