//! The helpful bootstrap APIs which enable an easy implementation of typical client side and server side pipeline initialization.

use bytes::BytesMut;
use log::{trace, warn};
use std::{
    cell::RefCell,
    io::Error,
    net::SocketAddr,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::ToSocketAddrs,
    sync::{Notify, broadcast},
};
use wg::AsyncWaitGroup;

use sansio::{RcInboundPipeline, RcOutboundPipeline, RcPipeline};
use sansio_executor::spawn_local;
use sansio_transport::{TaggedBytesMut, TransportContext};

mod bootstrap_tcp;
mod bootstrap_udp;

pub use bootstrap_tcp::{
    bootstrap_tcp_client::BootstrapTcpClient, bootstrap_tcp_server::BootstrapTcpServer,
};
pub use bootstrap_udp::{
    bootstrap_udp_client::BootstrapUdpClient, bootstrap_udp_server::BootstrapUdpServer,
};

/// A pipeline bundled with its write notification mechanism
pub(crate) struct PipelineWithNotify<R, W> {
    pub(crate) pipeline: Rc<RcPipeline<R, W>>,
    pub(crate) write_notify: Arc<Notify>,
}

impl<R, W> PipelineWithNotify<R, W> {
    /// Create a new pipeline with automatic write notification setup
    pub(crate) fn new(pipeline: Rc<RcPipeline<R, W>>) -> Self
    where
        R: 'static,
        W: 'static,
    {
        let write_notify = Arc::new(Notify::new());
        let notify_clone = write_notify.clone();

        pipeline.set_write_notify(Arc::new(move || {
            notify_clone.notify_one();
        }));

        Self {
            pipeline,
            write_notify,
        }
    }

    /// Get a reference to the pipeline for outbound operations
    pub(crate) fn pipeline(&self) -> &Rc<RcPipeline<R, W>> {
        &self.pipeline
    }
}

/// Creates a new [RcPipeline]
pub type PipelineFactoryFn<R, W> =
    Box<dyn Fn(SocketAddr, Option<SocketAddr>) -> Rc<RcPipeline<R, W>>>;

const DEFAULT_TIMEOUT_DURATION: Duration = Duration::from_secs(86400); // 1 day duration

struct Bootstrap<W> {
    max_payload_size: usize,
    pipeline_factory_fn: Option<Rc<PipelineFactoryFn<TaggedBytesMut, W>>>,
    close_tx: Rc<RefCell<Option<broadcast::Sender<()>>>>,
    wg: Rc<RefCell<Option<AsyncWaitGroup>>>,
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
            let _ = close_tx.send(());
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
