/// retty-io Driver.
// #[cfg(unix)]
pub(crate) mod op;
pub(crate) mod shared_fd;
#[cfg(feature = "sync")]
pub(crate) mod thread;

mod legacy;

mod util;

use std::{
    io,
    task::{Context, Poll},
    time::Duration,
};

pub use self::legacy::LegacyDriver;
// #[cfg(windows)]
// pub mod op {
//     pub struct CompletionMeta {}
//     pub struct Op<T> {
//         pub data: T,
//     }
//     pub trait OpAble {}
// }
use self::legacy::LegacyInner;
use self::op::{CompletionMeta, Op, OpAble};

/// Unpark a runtime of another thread.
pub(crate) mod unpark {
    #[allow(unreachable_pub)]
    pub trait Unpark: Sync + Send + 'static {
        /// Unblocks a thread that is blocked by the associated `Park` handle.
        ///
        /// Calling `unpark` atomically makes available the unpark token, if it
        /// is not already available.
        ///
        /// # Panics
        ///
        /// This function **should** not panic, but ultimately, panics are left
        /// as an implementation detail. Refer to the documentation for
        /// the specific `Unpark` implementation
        fn unpark(&self) -> std::io::Result<()>;
    }
}

impl unpark::Unpark for Box<dyn unpark::Unpark> {
    fn unpark(&self) -> io::Result<()> {
        (**self).unpark()
    }
}

impl unpark::Unpark for std::sync::Arc<dyn unpark::Unpark> {
    fn unpark(&self) -> io::Result<()> {
        (**self).unpark()
    }
}

/// Core driver trait.
pub trait Driver {
    /// Run with driver TLS.
    fn with<R>(&self, f: impl FnOnce() -> R) -> R;
    /// Submit ops to kernel and process returned events.
    fn submit(&self) -> io::Result<()>;
    /// Wait infinitely and process returned events.
    fn park(&self) -> io::Result<()>;
    /// Wait with timeout and process returned events.
    fn park_timeout(&self, duration: Duration) -> io::Result<()>;

    /// The struct to wake thread from another.
    #[cfg(feature = "sync")]
    type Unpark: unpark::Unpark;

    /// Get Unpark.
    #[cfg(feature = "sync")]
    fn unpark(&self) -> Self::Unpark;
}

scoped_thread_local!(pub(crate) static CURRENT: Inner);

pub(crate) enum Inner {
    Legacy(std::rc::Rc<std::cell::UnsafeCell<LegacyInner>>),
}

impl Inner {
    fn submit_with<T: OpAble>(&self, data: T) -> io::Result<Op<T>> {
        match self {
            Inner::Legacy(this) => LegacyInner::submit_with_data(this, data),
        }
    }

    #[allow(unused)]
    fn poll_op<T: OpAble>(
        &self,
        data: &mut T,
        index: usize,
        cx: &mut Context<'_>,
    ) -> Poll<CompletionMeta> {
        match self {
            Inner::Legacy(this) => LegacyInner::poll_op::<T>(this, data, cx),
        }
    }

    #[allow(unused)]
    fn drop_op<T: 'static>(&self, index: usize, data: &mut Option<T>) {
        match self {
            Inner::Legacy(_) => {}
        }
    }

    #[allow(unused)]
    pub(super) unsafe fn cancel_op(&self, op_canceller: &op::OpCanceller) {
        match self {
            Inner::Legacy(this) => {
                if let Some(direction) = op_canceller.direction {
                    LegacyInner::cancel_op(this, op_canceller.index, direction)
                }
            }
        }
    }
}

/// The unified UnparkHandle.
#[cfg(feature = "sync")]
#[derive(Clone)]
pub(crate) enum UnparkHandle {
    Legacy(self::legacy::UnparkHandle),
}

#[cfg(feature = "sync")]
impl unpark::Unpark for UnparkHandle {
    fn unpark(&self) -> io::Result<()> {
        match self {
            UnparkHandle::Legacy(inner) => inner.unpark(),
        }
    }
}

#[cfg(feature = "sync")]
impl From<self::legacy::UnparkHandle> for UnparkHandle {
    fn from(inner: self::legacy::UnparkHandle) -> Self {
        Self::Legacy(inner)
    }
}
