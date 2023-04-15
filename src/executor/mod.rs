//! Async executors.

use core_affinity::{set_for_current, CoreId};
use scoped_tls::scoped_thread_local;
use smol::{LocalExecutor, Task};
use std::{
    future::Future,
    io::Result,
    thread::{self, JoinHandle},
};

scoped_thread_local!(static LOCAL_EX: LocalExecutor<'_>);

/// A factory that can be used to configure and create a [`LocalExecutor`].
#[derive(Debug, Default)]
pub struct LocalExecutorBuilder {
    core_id: Option<CoreId>,
    name: String,
}

impl LocalExecutorBuilder {
    /// Creates a new LocalExecutorBuilder
    pub fn new() -> Self {
        Self::default()
    }

    /// Names the thread-to-be. Currently, the name is used for identification only in panic messages.
    pub fn name(mut self, name: &str) -> Self {
        self.name = String::from(name);
        self
    }

    /// Pins the thread to the specified CPU core
    pub fn core_id(mut self, core_id: CoreId) -> Self {
        self.core_id = Some(core_id);
        self
    }

    /// Runs the local executor on the current thread until the given future completes.
    pub fn run<T>(mut self, f: impl Future<Output = T>) -> T {
        if let Some(core_id) = self.core_id.take() {
            set_for_current(core_id);
        }

        let local_ex = LocalExecutor::new();
        LOCAL_EX.set(&local_ex, || {
            futures_lite::future::block_on(local_ex.run(f))
        })
    }

    /// Spawns a thread to run the local executor until the given future completes.
    pub fn spawn<G, F, T>(mut self, fut_gen: G) -> Result<JoinHandle<T>>
    where
        G: FnOnce() -> F + Send + 'static,
        F: Future<Output = T> + 'static,
        T: Send + 'static,
    {
        let mut core_id = self.core_id.take();

        thread::Builder::new().name(self.name).spawn(move || {
            if let Some(core_id) = core_id.take() {
                set_for_current(core_id);
            }

            let local_ex = LocalExecutor::new();
            LOCAL_EX.set(&local_ex, || {
                futures_lite::future::block_on(local_ex.run(fut_gen()))
            })
        })
    }
}

/// Spawns a task onto the current single-threaded executor.
///
/// If called from a [`LocalExecutor`], the task is spawned on it.
/// Otherwise, this method panics.
pub fn spawn_local<T: 'static>(future: impl Future<Output = T> + 'static) -> Task<T> {
    if LOCAL_EX.is_set() {
        LOCAL_EX.with(|local_ex| local_ex.spawn(future))
    } else {
        panic!("`spawn_local()` must be called from a `LocalExecutor`")
    }
}

/// Attempts to yield local to run a task if at least one is scheduled.
/// Running a scheduled task means simply polling its future once.
pub fn try_yield_local() -> bool {
    if LOCAL_EX.is_set() {
        LOCAL_EX.with(|local_ex| local_ex.try_tick())
    } else {
        panic!("`try_yield_local()` must be called from a `LocalExecutor`")
    }
}

/// Yield local to run other tasks until there is no other pending task.
pub fn yield_local() {
    if LOCAL_EX.is_set() {
        LOCAL_EX.with(|local_ex| while local_ex.try_tick() {})
    } else {
        panic!("`try_yield_local()` must be called from a `LocalExecutor`")
    }
}
