use super::*;

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for UDP servers.
pub struct BootstrapUdpServer<W> {
    internal: BootstrapUdp<W>,
}

impl<W: 'static> Default for BootstrapUdpServer<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: 'static> BootstrapUdpServer<W> {
    /// Creates a new BootstrapUdpServer
    pub fn new() -> Self {
        Self {
            internal: BootstrapUdp::new(),
        }
    }

    /// Set IOThreadPoolExecutor for io_group
    pub fn io_group(&mut self /*TODO: io_group: IOThreadPoolExecutor*/) -> &mut Self {
        self
    }

    /// Creates pipeline instances from when calling [BootstrapUdpServer::bind].
    pub fn pipeline(
        &mut self,
        pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>,
    ) -> &mut Self {
        self.internal.pipeline(pipeline_factory_fn);
        self
    }

    /// Binds local address and port
    pub async fn bind<A: AsyncToSocketAddrs>(&mut self, addr: A) -> Result<SocketAddr, Error> {
        let local_addr = self.internal.bind(addr).await?;
        let peer_addr: Option<SocketAddr> = None;
        self.internal.connect(peer_addr).await?;
        Ok(local_addr)
    }

    /// Gracefully stop the BootstrapUdpServer
    pub async fn stop(&self) {
        self.internal.stop().await
    }
}
