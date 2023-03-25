use scoped_tls::scoped_thread_local;
use smol::{LocalExecutor, Task};
use std::future::Future;

scoped_thread_local!(static LOCAL_EX: LocalExecutor<'_>);

/// Runs the local executor until the given future completes.
pub fn run_local<T>(f: impl Future<Output = T>) -> T {
    let local_ex = LocalExecutor::default();
    LOCAL_EX.set(&local_ex, || {
        futures_lite::future::block_on(local_ex.run(f))
    })
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
