# sansio-codec

Protocol codecs for the `sansio` ecosystem, providing reusable frame decoders, string codecs, and transport abstractions.

## Features

- **Frame Decoders**: Line-based frame decoders for parsing delimited streams
- **String Codecs**: UTF-8 string encoding/decoding
- **Transport Abstractions**: Tagged message types with transport metadata (IP addresses, ports, protocol type, ECN)
- **Sans-IO Design**: Pure protocol logic without I/O dependencies

## Quick Start

```toml
[dependencies]
sansio-codec = "0.0.8"
```

## Usage

### Building a Pipeline with Codecs

```rust
use sansio::Pipeline;
use sansio_codec::{
    LineBasedFrameDecoder, TaggedByteToMessageCodec,
    TaggedStringCodec, TerminatorType
};
use std::rc::Rc;

let pipeline = Rc::new(Pipeline::new());

// Add line-based frame decoder (splits on \n or \r\n)
let frame_decoder = TaggedByteToMessageCodec::new(Box::new(
    LineBasedFrameDecoder::new(8192, true, TerminatorType::BOTH)
));
pipeline.add_back(frame_decoder);

// Add string codec (converts bytes to UTF-8 strings)
pipeline.add_back(TaggedStringCodec::new());

// Add your business logic handler
// pipeline.add_back(your_handler);

pipeline.update();
```

### Transport Context

All messages are wrapped with transport metadata:

```rust
use sansio_codec::{TransportContext, TaggedBytesMut, TransportProtocol};
use std::time::Instant;
use bytes::BytesMut;

let msg = TaggedBytesMut {
    now: Instant::now(),
    transport: TransportContext {
        local_addr: "127.0.0.1:8080".parse().unwrap(),
        peer_addr: "127.0.0.1:1234".parse().unwrap(),
        transport_protocol: TransportProtocol::TCP,
        ecn: None,
    },
    message: BytesMut::from("Hello"),
};
```

## Modules

### `byte_to_message_decoder`

Frame decoders for parsing byte streams:

- **`LineBasedFrameDecoder`**: Splits frames on line terminators (`\n`, `\r\n`, or both)
- **`TaggedByteToMessageCodec`**: Handler wrapper for any `MessageDecoder`
- **`TerminatorType`**: Configure line terminator type (BOTH, NEWLINE, CarriageNewline)

### `string_codec`

String encoding/decoding:

- **`TaggedStringCodec`**: Converts between `TaggedBytesMut` and `TaggedString`

### `transport`

Transport abstractions:

- **`TransportContext`**: Local/peer addresses, transport protocol type, ECN bits
- **`TaggedBytesMut`**: BytesMut message with transport context
- **`TaggedString`**: String message with transport context
- **`TransportProtocol`**: TCP or UDP
- **`FourTuple`**: Local and peer addresses
- **`FiveTuple`**: Local address, peer address, and transport protocol

## Example

See the [examples](../examples) directory for complete working examples using these codecs, including:
- TCP chat server with line-based framing
- UDP chat server with message broadcasting

## Documentation

- [API Documentation](https://docs.rs/sansio-codec)
- [Main sansio crate](https://docs.rs/sansio)
- [Executor (sansio-executor)](https://docs.rs/sansio-executor)

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
