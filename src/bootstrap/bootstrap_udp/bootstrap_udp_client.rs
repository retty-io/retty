use super::*;

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for UDP clients.
pub struct BootstrapUdpClient<W> {
    bootstrap_udp: BootstrapUdp<W>,
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
            bootstrap_udp: BootstrapUdp::new(),
        }
    }

    /// Set ThreadPool for io_group
    pub fn io_group(&mut self, io_group: ThreadPool) -> &mut Self {
        self.bootstrap_udp.io_group(io_group);
        self
    }

    /// Creates pipeline instances from when calling [BootstrapUdpClient::bind].
    pub fn pipeline(
        &mut self,
        pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>,
    ) -> &mut Self {
        self.bootstrap_udp.pipeline(pipeline_factory_fn);
        self
    }

    /// Binds local address and port
    pub async fn bind<A: AsyncToSocketAddrs>(&mut self, addr: A) -> Result<SocketAddr, Error> {
        self.bootstrap_udp.bind(addr).await
    }

    /// Connects to the remote peer
    pub async fn connect(
        &mut self,
        addr: SocketAddr,
    ) -> Result<Rc<dyn OutboundPipeline<W>>, Error> {
        self.bootstrap_udp.connect(Some(addr)).await
    }

    /// Gracefully stop the BootstrapUdpClient
    pub async fn stop(&self) {
        self.bootstrap_udp.stop().await
    }
}
