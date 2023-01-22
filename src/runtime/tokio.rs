use std::{future::Future, pin::Pin};

use super::Runtime;

/// A runtime for tokio
#[derive(Debug)]
pub struct TokioRuntime;

impl Runtime for TokioRuntime {
    fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send>>) {
        tokio::spawn(future);
    }
}

/// A wrapper for tokio sync mod
pub mod sync {
    pub use tokio::sync::Mutex;
}

/// A wrapper for tokio net mod
pub mod net {
    pub use tokio::net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpStream, ToSocketAddrs, UdpSocket,
    };
}

/// A wrapper for tokio io mod
pub mod io {
    pub use tokio::io::{AsyncReadExt, AsyncWriteExt};
}

/// A wrapper for tokio mpsc mod
pub mod mpsc {
    pub use tokio::sync::broadcast::{Receiver, Sender};
    /// A bounded wrapper for tokio broadcast channel
    pub fn bounded<T: Clone>(cap: usize) -> (Sender<T>, Receiver<T>) {
        tokio::sync::broadcast::channel(cap)
    }
}

pub use tokio::time::sleep;
