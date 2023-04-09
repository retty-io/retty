use super::*;

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for UDP clients.
pub struct BootstrapUdpClient<W> {
    internal: BootstrapUdp<W>,
}

impl<W: 'static> Default for BootstrapUdpClient<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: 'static> BootstrapUdpClient<W> {
    /// Creates a new BootstrapUdpClient
    pub fn new() -> Self {
        Self {
            internal: BootstrapUdp::new(),
        }
    }

    /// Set IOThreadPoolExecutor for io_group
    pub fn io_group(&mut self /*TODO: io_group: IOThreadPoolExecutor*/) -> &mut Self {
        self
    }

    /// Creates pipeline instances from when calling [BootstrapUdpClient::bind].
    pub fn pipeline(
        &mut self,
        pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>,
    ) -> &mut Self {
        self.internal.pipeline(pipeline_factory_fn);
        self
    }

    /// Binds local address and port
    pub async fn bind<A: AsyncToSocketAddrs>(&mut self, addr: A) -> Result<SocketAddr, Error> {
        self.internal.bind(addr).await
    }

    /// Connects to the remote peer
    pub async fn connect(
        &mut self,
        addr: SocketAddr,
    ) -> Result<Rc<dyn OutboundPipeline<W>>, Error> {
        self.internal.connect(Some(addr)).await
    }

    /// Gracefully stop the BootstrapUdpClient
    pub async fn stop(&self) {
        self.internal.stop().await
    }
}
