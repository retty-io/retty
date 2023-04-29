//! The helpful bootstrap APIs which enable an easy implementation of typical client side and server side pipeline initialization.

use bytes::BytesMut;
use futures_lite::{AsyncReadExt, AsyncWriteExt};
use local_sync::mpsc::{unbounded::channel, unbounded::Rx as LocalReceiver};
use log::{trace, warn};
use smol::{net::AsyncToSocketAddrs, Timer};
use std::{
    cell::RefCell,
    io::Error,
    net::SocketAddr,
    rc::Rc,
    time::{Duration, Instant},
};
use waitgroup::{WaitGroup, Worker};

use crate::channel::{InboundPipeline, OutboundPipeline, Pipeline};
use crate::executor::spawn_local;
use crate::transport::{AsyncTransportWrite, TaggedBytesMut, TransportContext};

mod bootstrap_tcp;
mod bootstrap_udp;

pub use bootstrap_tcp::{
    bootstrap_tcp_client::BootstrapTcpClient, bootstrap_tcp_server::BootstrapTcpServer,
};
pub use bootstrap_udp::{
    bootstrap_udp_client::BootstrapUdpClient, bootstrap_udp_server::BootstrapUdpServer,
};

/// Creates a new [Pipeline]
pub type PipelineFactoryFn<R, W> = Box<dyn (Fn(AsyncTransportWrite<R>) -> Rc<Pipeline<R, W>>)>;

const MAX_DURATION_IN_SECS: u64 = 86400; // 1 day

struct Bootstrap<W> {
    max_payload_size: usize,
    pipeline_factory_fn: Option<Rc<PipelineFactoryFn<TaggedBytesMut, W>>>,
    close_tx: Rc<RefCell<Option<async_broadcast::Sender<()>>>>,
    wg: Rc<RefCell<Option<WaitGroup>>>,
}

impl<W: 'static> Default for Bootstrap<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: 'static> Bootstrap<W> {
    fn new() -> Self {
        Self {
            max_payload_size: 2048, // Typical internet MTU = 1500, rounded up to a power of 2
            pipeline_factory_fn: None,
            close_tx: Rc::new(RefCell::new(None)),
            wg: Rc::new(RefCell::new(None)),
        }
    }

    fn max_payload_size(&mut self, max_payload_size: usize) -> &mut Self {
        self.max_payload_size = max_payload_size;
        self
    }

    fn pipeline(&mut self, pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>) -> &mut Self {
        self.pipeline_factory_fn = Some(Rc::new(Box::new(pipeline_factory_fn)));
        self
    }

    async fn stop(&self) {
        let mut close_tx = self.close_tx.borrow_mut();
        if let Some(close_tx) = close_tx.take() {
            let _ = close_tx.try_broadcast(());
        }
    }

    async fn wait_for_stop(&self) {
        let wg = {
            let mut wg = self.wg.borrow_mut();
            wg.take()
        };
        if let Some(wg) = wg {
            wg.wait().await;
        }
    }

    async fn graceful_stop(&self) {
        self.stop().await;
        self.wait_for_stop().await;
    }
}
