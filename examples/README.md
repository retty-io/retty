# Sansio Examples

This crate contains examples demonstrating how to use the sansio ecosystem, including both the `sansio` core crate and the `sansio-executor` tokio-based executor.

## Structure

- **`src/helpers/`**: Reusable helper modules for examples (codecs, transport utilities)
- **`examples/`**: Example programs demonstrating various features

## Running Examples

```bash
cargo run --example chat_server_tcp -p examples
```

Or:

```bash
cargo run --example chat_server_udp -p examples
```

## Available Examples

### chat_server_tcp

A TCP-based chat server demonstrating:
- Tokio async TCP sockets with `tokio::select!`
- Pipeline-based protocol processing
- Handler composition (frame decoder, string codec)
- Shared state management
- Multi-client message broadcasting
- Task spawning and detaching with `spawn_local()`

**Run:**
```bash
cargo run --example chat_server_tcp -p examples -- --host 127.0.0.1 --port 8080
```

**Connect with netcat:**
```bash
nc 127.0.0.1 8080
```

### chat_server_udp

A UDP-based chat server demonstrating:
- Pipeline-based protocol processing
- Handler composition (frame decoder, string codec)
- Shared state management
- Multi-client message broadcasting
- Local executor integration

**Run:**
```bash
cargo run --example chat_server_udp -p examples -- --host 127.0.0.1 --port 5000
```

**Connect with netcat:**
```bash
nc -u 127.0.0.1 5000
```

## Helper Modules

The `src/helpers/` directory contains reusable components:

- **`byte_to_message_decoder`**: Frame decoders (line-based, length-prefixed)
- **`string_codec`**: String encoding/decoding
- **`transport`**: Transport abstractions and tagged message types

These helpers can be used in your own applications as examples of how to build protocol handlers.

## Adding New Examples

1. Create a new file in `examples/` directory
2. Use helpers from `examples::helpers` module
3. Add the example to `Cargo.toml`:

```toml
[[example]]
name = "my_example"
path = "examples/my_example.rs"
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT license ([LICENSE-MIT](../LICENSE-MIT))

at your option.
