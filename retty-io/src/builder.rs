use std::{io, marker::PhantomData};

use crate::driver::LegacyDriver;
use crate::{
    driver::Driver,
    time::{driver::TimeDriver, Clock},
    utils::thread_id::gen_id,
    Runtime,
};

// ===== basic builder structure definition =====

/// Runtime builder
pub struct RuntimeBuilder<D> {
    // iouring entries
    entries: Option<u32>,

    // blocking handle
    #[cfg(feature = "sync")]
    blocking_handle: crate::blocking::BlockingHandle,
    // driver mark
    _mark: PhantomData<D>,
}

scoped_thread_local!(pub(crate) static BUILD_THREAD_ID: usize);

impl<T> Default for RuntimeBuilder<T> {
    /// Create a default runtime builder
    #[must_use]
    fn default() -> Self {
        RuntimeBuilder::<T>::new()
    }
}

impl<T> RuntimeBuilder<T> {
    /// Create a default runtime builder
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: None,

            #[cfg(feature = "sync")]
            blocking_handle: crate::blocking::BlockingStrategy::Panic.into(),
            _mark: PhantomData,
        }
    }
}

// ===== buildable trait and forward methods =====

/// Buildable trait.
pub trait Buildable: Sized {
    /// Build the runtime.
    fn build(this: &RuntimeBuilder<Self>) -> io::Result<Runtime<Self>>;
}

#[allow(unused)]
macro_rules! direct_build {
    ($ty: ty) => {
        impl RuntimeBuilder<$ty> {
            /// Build the runtime.
            pub fn build(&self) -> io::Result<Runtime<$ty>> {
                Buildable::build(self)
            }
        }
    };
}

direct_build!(LegacyDriver);
direct_build!(TimeDriver<LegacyDriver>);

// ===== builder impl =====

impl Buildable for LegacyDriver {
    fn build(this: &RuntimeBuilder<Self>) -> io::Result<Runtime<LegacyDriver>> {
        let thread_id = gen_id();
        #[cfg(feature = "sync")]
        let blocking_handle = this.blocking_handle.clone();

        BUILD_THREAD_ID.set(&thread_id, || {
            let driver = match this.entries {
                Some(entries) => LegacyDriver::new_with_entries(entries)?,
                None => LegacyDriver::new()?,
            };
            #[cfg(feature = "sync")]
            let context = crate::runtime::Context::new(blocking_handle);
            #[cfg(not(feature = "sync"))]
            let context = crate::runtime::Context::new();
            Ok(Runtime { driver, context })
        })
    }
}

impl<D> RuntimeBuilder<D> {
    const MIN_ENTRIES: u32 = 256;

    /// Set io_uring entries, min size is 256 and the default size is 1024.
    #[must_use]
    pub fn with_entries(mut self, entries: u32) -> Self {
        // If entries is less than 256, it will be 256.
        if entries < Self::MIN_ENTRIES {
            self.entries = Some(Self::MIN_ENTRIES);
            return self;
        }
        self.entries = Some(entries);
        self
    }
}

// ===== enable_timer related =====
mod time_wrap {
    pub trait TimeWrapable {}
}

impl time_wrap::TimeWrapable for LegacyDriver {}

impl<D: Driver> Buildable for TimeDriver<D>
where
    D: Buildable,
{
    /// Build the runtime
    fn build(this: &RuntimeBuilder<Self>) -> io::Result<Runtime<TimeDriver<D>>> {
        let Runtime {
            driver,
            mut context,
        } = Buildable::build(&RuntimeBuilder::<D> {
            entries: this.entries,
            #[cfg(feature = "sync")]
            blocking_handle: this.blocking_handle.clone(),
            _mark: PhantomData,
        })?;

        let timer_driver = TimeDriver::new(driver, Clock::new());
        context.time_handle = Some(timer_driver.handle.clone());
        Ok(Runtime {
            driver: timer_driver,
            context,
        })
    }
}

impl<D: time_wrap::TimeWrapable> RuntimeBuilder<D> {
    /// Enable all(currently only timer)
    #[must_use]
    pub fn enable_all(self) -> RuntimeBuilder<TimeDriver<D>> {
        self.enable_timer()
    }

    /// Enable timer
    #[must_use]
    pub fn enable_timer(self) -> RuntimeBuilder<TimeDriver<D>> {
        let Self {
            entries,
            #[cfg(feature = "sync")]
            blocking_handle,
            ..
        } = self;
        RuntimeBuilder {
            entries,
            #[cfg(feature = "sync")]
            blocking_handle,
            _mark: PhantomData,
        }
    }

    /// Attach thread pool, this will overwrite blocking strategy.
    /// All `spawn_blocking` will be executed on given thread pool.
    #[cfg(feature = "sync")]
    #[must_use]
    pub fn attach_thread_pool(
        mut self,
        tp: std::sync::Arc<dyn crate::blocking::ThreadPool>,
    ) -> Self {
        self.blocking_handle = crate::blocking::BlockingHandle::Attached(tp);
        self
    }

    /// Set blocking strategy, this will overwrite thread pool setting.
    /// If `BlockingStrategy::Panic` is used, it will panic if `spawn_blocking` on this thread.
    /// If `BlockingStrategy::ExecuteLocal` is used, it will execute with current thread, and may
    /// cause tasks high latency.
    /// Attaching a thread pool is recommended if `spawn_blocking` will be used.
    #[cfg(feature = "sync")]
    #[must_use]
    pub fn with_blocking_strategy(mut self, strategy: crate::blocking::BlockingStrategy) -> Self {
        self.blocking_handle = crate::blocking::BlockingHandle::Empty(strategy);
        self
    }
}
