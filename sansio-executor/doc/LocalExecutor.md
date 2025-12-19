# Local Executor Documentation

## Table of Contents

1. [Overview](#overview)
2. [Quick Start](#quick-start)
3. [Feature Flags](#feature-flags)
4. [API Reference](#api-reference)
5. [API Differences Between Runtimes](#api-differences-between-runtimes)
6. [Migration Guide](#migration-guide)
7. [Implementation Details](#implementation-details)
8. [Design Decisions](#design-decisions)
9. [File Structure](#file-structure)

---

## Overview

The `sansio` crate provides a local executor abstraction that can be backed by either **smol** or **tokio**, depending on your needs. This allows you to write protocol logic that is runtime-agnostic, while still getting the benefits of each runtime's strengths.

### Key Features

- **Runtime Flexibility**: Choose between smol or tokio at compile time
- **CPU Pinning**: Pin executor threads to specific CPU cores
- **Thread Naming**: Name executor threads for debugging
- **Consistent API**: Similar API across both runtimes (with documented differences)

---

## Quick Start

### With Default (smol) Runtime

```toml
[dependencies]
sansio = "0.0.5"
```

```rust
use sansio::LocalExecutorBuilder;

fn main() {
    LocalExecutorBuilder::default()
        .run(async {
            println!("Running on smol!");
        });
}
```

### With Tokio Runtime

```toml
[dependencies]
sansio = { version = "0.0.5", default-features = false, features = ["runtime-tokio"] }
```

```rust
use sansio::LocalExecutorBuilder;

fn main() {
    LocalExecutorBuilder::default()
        .run(async {
            println!("Running on tokio!");
        });
}
```

### With CPU Pinning

```rust
use sansio::LocalExecutorBuilder;
use core_affinity::CoreId;

fn main() {
    LocalExecutorBuilder::new()
        .name("my-executor")
        .core_id(CoreId { id: 0 })
        .run(async {
            println!("Running on CPU core 0!");
        });
}
```

### Spawning Local Tasks

```rust
use sansio::{LocalExecutorBuilder, spawn_local};

fn main() {
    LocalExecutorBuilder::default().run(async {
        let task1 = spawn_local(async {
            println!("Task 1");
            42
        });

        let task2 = spawn_local(async {
            println!("Task 2");
            100
        });

        let result1 = task1.await;
        let result2 = task2.await;

        println!("Results: {}, {}", result1, result2);
    });
}
```

---

## Feature Flags

### Available Features

- **`runtime-smol`** (default): Use smol's LocalExecutor
  - Lightweight and minimal dependencies
  - Excellent for simple use cases

- **`runtime-tokio`**: Use tokio's LocalSet
  - Full tokio ecosystem integration
  - Better for complex scenarios

### Important Notes

- You **must** enable exactly one runtime feature
- Cannot enable both features simultaneously (compile error)
- Cannot disable all features (compile error)

### Examples

```toml
# Default: Using smol
[dependencies]
sansio = "0.0.5"

# Explicitly using smol
[dependencies]
sansio = { version = "0.0.5", features = ["runtime-smol"] }

# Using tokio
[dependencies]
sansio = { version = "0.0.5", default-features = false, features = ["runtime-tokio"] }
```

---

## API Reference

### `LocalExecutorBuilder`

A factory for configuring and creating a local executor.

#### Methods

- **`new()`** - Creates a new LocalExecutorBuilder
- **`name(name: &str)`** - Sets the thread name (used in panic messages)
- **`core_id(core_id: CoreId)`** - Pins the thread to a specific CPU core
- **`run<T>(future: impl Future<Output = T>) -> T`** - Runs the executor on the current thread
- **`spawn<G, F, T>(fut_gen: G) -> Result<JoinHandle<T>>`** - Spawns a new thread to run the executor

#### Example

```rust
use sansio::LocalExecutorBuilder;

LocalExecutorBuilder::new()
    .name("worker")
    .run(async {
        // Your async code here
    });
```

### `spawn_local()`

Spawns a task onto the current single-threaded executor.

**Signature:**
- Smol: `pub fn spawn_local<T>(future: impl Future<Output = T>) -> Task<T>`
- Tokio: `pub fn spawn_local<T>(future: impl Future<Output = T>) -> JoinHandle<T>`

**Panics:** If not called from within a LocalExecutor/LocalSet context.

#### Example

```rust
use sansio::spawn_local;

let task = spawn_local(async {
    // Your async work
    42
});

let result = task.await;
```

### `yield_local()`

Yields execution to allow other tasks in the same executor to run.

**Signature:**
- Smol: `pub async fn yield_local()` (asynchronous)
- Tokio: `pub async fn yield_local()` (asynchronous)

Both implementations provide a consistent async API for yielding to other tasks.

#### Example

```rust
use sansio::yield_local;

// Works identically in both smol and tokio
yield_local().await;
```

---

## API Differences Between Runtimes

The sansio crate provides a consistent API across both smol and tokio runtimes.

### Summary Table

| API | Smol | Tokio | Notes |
|-----|------|-------|-------|
| `LocalExecutorBuilder::new()` | ✅ | ✅ | Identical |
| `LocalExecutorBuilder::name()` | ✅ | ✅ | Identical |
| `LocalExecutorBuilder::core_id()` | ✅ | ✅ | Identical |
| `LocalExecutorBuilder::run()` | ✅ | ✅ | Identical |
| `LocalExecutorBuilder::spawn()` | ✅ | ✅ | Identical |
| `spawn_local()` | Returns `Task<T>` | Returns `JoinHandle<T>` | Different handle types |
| `yield_local()` | **Async** | **Async** | **Identical API** |
| `try_yield_local()` | ❌ Removed | ❌ Removed | Removed from both |

### Implementation Details

While the APIs are identical, the implementations differ internally:

#### `yield_local()` Implementation

**Smol Runtime:**
```rust
pub async fn yield_local()  // Async function
```

**Behavior:**
- Internally uses `LocalExecutor::try_tick()` to run pending tasks
- Runs synchronously within the async wrapper
- Provides consistent async API

**Tokio Runtime:**
```rust
pub async fn yield_local()  // Async function
```

**Behavior:**
- Uses `tokio::task::yield_now().await` internally
- Truly asynchronous task yielding
- Same API as smol

**Result:** Both runtimes now provide identical APIs with `yield_local().await`, making code fully portable.

---

## Migration Guide

### For Existing Smol Users

If you were using the old synchronous `yield_local()` in smol:

```rust
// Old (synchronous)
yield_local();

// New (async - add .await)
yield_local().await;
```

### For Users Only Using Tokio

No changes needed - the API remains the same.

### For Cross-Runtime Code

Great news! Code now works identically across both runtimes without conditional compilation:

```rust
use sansio::{LocalExecutorBuilder, spawn_local, yield_local};

// This code works identically in both smol and tokio!
LocalExecutorBuilder::default().run(async {
    spawn_local(async {
        println!("Task starting");
        yield_local().await;  // Same in both runtimes
        println!("Task resuming");
    });
});
```

**No conditional compilation needed!** The `yield_local().await` syntax works identically in both runtimes.

### Recommendations

**For new code:** Simply use `yield_local().await` - it works the same across both runtimes.

**For existing smol code:** Add `.await` to `yield_local()` calls:
- Old: `yield_local();`
- New: `yield_local().await;`

**For existing tokio code:** No changes required.

---

## Implementation Details

### File Structure

The local executor implementation is split into separate files for clarity:

```
src/local_executor/
├── mod.rs     (80 lines)  - Module root, docs, conditional compilation
├── smol.rs    (93 lines)  - Smol runtime implementation
└── tokio.rs   (125 lines) - Tokio runtime implementation
```

**Benefits:**
- Each runtime isolated in its own file
- Cleaner code organization
- Easier to maintain and review
- Better IDE/tooling support
- No nested feature-gated blocks

### Smol Implementation (`smol.rs`)

Uses smol's `LocalExecutor`:
- Thread-local storage via `scoped_tls`
- Blocks on `futures_lite::future::block_on()`
- Full support for synchronous task polling via `try_tick()`

### Tokio Implementation (`tokio.rs`)

Uses tokio's `LocalSet`:
- Creates `tokio::runtime::Builder::new_current_thread()`
- Blocks on `LocalSet::run_until()`
- Async-only task yielding

### Common Features

Both implementations provide:
- CPU core pinning via `core_affinity`
- Thread naming
- Builder pattern API
- Thread spawning with `spawn()`

---

## Design Decisions

### 1. API Uniformity Over Implementation Details

We prioritize **API consistency** across runtimes:

- **`yield_local()`**: Same async signature in both runtimes, even though implementations differ internally
- Better than different APIs where one runtime behaves differently
- Makes code portable between runtimes

**Rationale:** A consistent async API is more valuable than exposing implementation differences.

### 2. Removal of `try_yield_local()`

We removed `try_yield_local()` from both runtimes because:

- **API Consistency**: Having it only in smol would create an awkward split
- **Limited Usefulness**: Most async code doesn't need explicit task polling
- **Tokio Can't Implement It**: No equivalent to `try_tick()`
- **Better Patterns**: Encourages event-driven async code

Users who absolutely need this can:
- Access smol's `LocalExecutor::try_tick()` directly
- Restructure code to use event-driven patterns (recommended)

### 3. Compile-Time Safety

Missing functionality causes compile errors rather than runtime failures:

- Using invalid feature combinations causes compile error
- Using removed functions causes compile error
- Better to fail fast than silently

### 4. Simplicity

We removed `try_yield_local()` from both runtimes because:

- **API Consistency**: Having it only in smol would create an awkward split
- **Limited Usefulness**: Most async code doesn't need explicit task polling
- **Tokio Can't Implement It**: No equivalent to `try_tick()`
- **Better Patterns**: Encourages event-driven async code

Users who absolutely need this can:
- Access smol's `LocalExecutor::try_tick()` directly
- Restructure code to use event-driven patterns (recommended)

### 4. Correctness

Both implementations provide semantically correct behavior:

- `yield_local()` actually yields to other local tasks in both runtimes
- No misleading APIs
- Clear documentation of implementation differences

### Why `yield_local()` is Async

We made `yield_local()` async in **both** runtimes for API consistency:

| Approach | Result |
|----------|--------|
| Smol sync, Tokio async | ❌ Inconsistent API, requires conditional compilation |
| Both sync | ❌ Impossible - tokio only supports async yielding |
| **Both async** | ✅ **Consistent API, works everywhere** |

Making it async in both runtimes provides:
- ✅ Consistent API across runtimes
- ✅ Portable code (no conditional compilation needed)
- ✅ Correct functionality (yields to local tasks in both)
- ✅ Clear, predictable behavior

**Implementation differences** (transparent to users):
- **Smol**: Wraps synchronous `try_tick()` in async function
- **Tokio**: Uses `tokio::task::yield_now().await` natively

**Result**: Same API, same behavior, different internal implementation.

### Why `try_yield_local()` Was Removed

Originally we considered keeping it for smol only, but decided to remove from both:

**Tokio can't implement it because:**
- No API to query if tasks are ready
- No API to poll a single task
- No way to report whether work was done
- Opaque scheduler by design

**Removed from smol too because:**
- Better API consistency across runtimes
- Most code doesn't need it
- Encourages better async patterns
- Advanced users can access `LocalExecutor::try_tick()` directly

---

## File Structure

### Before Refactoring

```
src/
└── local_executor.rs  (260 lines, all implementations)
```

- Single file with nested modules
- Feature gates throughout
- Mixed documentation

### After Refactoring

```
src/
└── local_executor/
    ├── mod.rs     (80 lines)  - Module root, docs, feature gates
    ├── smol.rs    (100 lines) - Pure smol implementation
    └── tokio.rs   (125 lines) - Pure tokio implementation
```

### Benefits of New Structure

1. **Clarity**
   - Each runtime in its own file
   - Clear separation of concerns
   - Easy to locate and modify

2. **Navigation**
   - No scrolling through feature gates
   - IDE shows clean file structure
   - Better autocomplete

3. **Development Workflow**
   - Edit smol or tokio independently
   - Can't accidentally affect other runtime
   - Cleaner git diffs

4. **Code Review**
   - PR diffs are focused
   - Changes clearly isolated
   - Easier to review

5. **Testing & Benchmarking**
   - Can have runtime-specific tests
   - Benchmarks clearly separated
   - Runtime issues are isolated

### Evolution Path

This structure makes it easy to add more runtimes:

```
local_executor/
├── mod.rs
├── smol.rs
├── tokio.rs
├── async_std.rs   ← Future addition
├── embassy.rs     ← Future addition
└── custom.rs      ← User-provided runtime
```

---

## When to Use Each Runtime

### Use `runtime-smol` when:
- You want a minimal, lightweight executor
- Your application is primarily sans-IO focused
- You prefer simpler dependencies
- You're building embedded or resource-constrained systems

### Use `runtime-tokio` when:
- You need to integrate with the tokio ecosystem
- You're already using tokio in other parts of your application
- You need advanced runtime features
- You're building complex network services

---

## Trade-offs

### API Uniformity vs Implementation Complexity

We chose **API uniformity**:
- `yield_local()` has the same async signature in both runtimes
- Makes code portable without conditional compilation
- Internal implementation differs (smol wraps sync `try_tick()`, tokio uses async `yield_now()`)
- Better developer experience at the cost of slightly more complex smol implementation

### Simplicity vs Features

We chose **simplicity** by removing `try_yield_local()`:
- Cleaner, more focused API
- Encourages better async patterns
- Advanced users can still access low-level APIs directly (smol's `try_tick()`)

### Portability vs Runtime-Specific Optimization

We chose **portability**:
- Uniform APIs enable write-once, run-anywhere code
- Slightly less optimal for smol (async wrapper around sync operation)
- Greatly improved developer experience and code maintainability

---

## Examples

See the `examples/` directory for complete working examples:
- `chat_server_udp.rs` - UDP chat server using the local executor

---

## Contributing

When adding a new runtime:

1. Create `src/local_executor/new_runtime.rs`
2. Implement `LocalExecutorBuilder`, `spawn_local()`, `yield_local()`
3. Add conditional import in `mod.rs`
4. Update `Cargo.toml` with the new feature
5. Update this documentation
6. Add examples and tests

---

## License

See the project root for license information.
