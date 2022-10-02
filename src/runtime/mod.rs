use std::{fmt::Debug, future::Future, pin::Pin, sync::Arc};

/// Abstracts I/O and timer operations for runtime independence
pub trait Runtime: Send + Sync + Debug + 'static {
    /// Drive `future` to completion in the background
    fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send>>);
}

/// Automatically select an appropriate runtime from those enabled at compile time
///
/// If `runtime-tokio` is enabled and this function is called from within a Tokio runtime context,
/// then `TokioRuntime` is returned. Otherwise, if `runtime-async-std` is enabled, `AsyncStdRuntime`
/// is returned. Otherwise, `None` is returned.
pub fn default_runtime() -> Option<Arc<dyn Runtime>> {
    #[cfg(feature = "runtime-tokio")]
    {
        if ::tokio::runtime::Handle::try_current().is_ok() {
            return Some(Arc::new(TokioRuntime));
        }
    }

    #[cfg(feature = "runtime-async-std")]
    {
        Some(Arc::new(AsyncStdRuntime))
    }

    #[cfg(not(feature = "runtime-async-std"))]
    None
}

#[cfg(feature = "runtime-tokio")]
mod tokio;
#[cfg(feature = "runtime-tokio")]
pub use self::tokio::io;
#[cfg(feature = "runtime-tokio")]
pub use self::tokio::mpsc;
#[cfg(feature = "runtime-tokio")]
pub use self::tokio::net;
#[cfg(feature = "runtime-tokio")]
pub use self::tokio::sync;
#[cfg(feature = "runtime-tokio")]
pub use self::tokio::TokioRuntime;

#[cfg(feature = "runtime-async-std")]
mod async_std;
#[cfg(feature = "runtime-async-std")]
pub use self::async_std::io;
#[cfg(feature = "runtime-async-std")]
pub use self::async_std::mpsc;
#[cfg(feature = "runtime-async-std")]
pub use self::async_std::net;
#[cfg(feature = "runtime-async-std")]
pub use self::async_std::sync;
#[cfg(feature = "runtime-async-std")]
pub use self::async_std::AsyncStdRuntime;
