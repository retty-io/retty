//! The helpful bootstrap APIs which enable an easy implementation of typical client side and server side pipeline initialization.

use crate::channel::{InboundPipeline, Pipeline};
use bytes::BytesMut;
use local_sync::mpsc::unbounded::Tx;
use log::{trace, warn};
use std::net::SocketAddr;
use std::rc::Rc;
use std::time::Instant;

pub(crate) mod bootstrap_client_tcp;
pub(crate) mod bootstrap_client_udp;
pub(crate) mod bootstrap_server_tcp;
pub(crate) mod bootstrap_server_udp;

use crate::transport::{TaggedBytesMut, TransportContext};
pub use bootstrap_client_tcp::BootstrapClientTcp;
pub use bootstrap_client_udp::BootstrapClientUdp;
pub use bootstrap_server_tcp::BootstrapServerTcp;
pub use bootstrap_server_udp::BootstrapServerUdp;

/// Creates a new [Pipeline]
pub type PipelineFactoryFn<R, W> = Box<dyn (Fn(Tx<R>) -> Rc<Pipeline<R, W>>)>;

fn process_packet(
    result: monoio::BufResult<(usize, SocketAddr), Vec<u8>>,
    pipeline: &Rc<dyn InboundPipeline<TaggedBytesMut>>,
    local_addr: SocketAddr,
) -> bool {
    let (res, buf) = result;
    match res {
        Err(err) if err.raw_os_error() == Some(125) => {
            // Canceled success
            //TODO: return _buf to buffer_pool
            return false;
        }
        // Canceled but executed
        Err(err) => {
            warn!("socket read error {}", err);
            return true;
        }
        Ok((n, peer_addr)) => {
            if n == 0 {
                pipeline.read_eof();
                return true;
            }

            trace!("socket read {} bytes", n);
            pipeline.read(TaggedBytesMut {
                now: Instant::now(),
                transport: TransportContext {
                    local_addr,
                    peer_addr,
                    ecn: None,
                },
                message: BytesMut::from(&buf[..n]),
            });
        }
    }

    false
}

const MAX_DURATION_IN_SECS: u64 = 86400; // 1 day
