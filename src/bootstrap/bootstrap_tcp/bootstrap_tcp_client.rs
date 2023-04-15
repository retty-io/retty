use super::*;

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for TCP clients.
pub struct BootstrapTcpClient<W> {
    bootstrap_tcp: BootstrapTcp<W>,
}

impl<W: 'static> Default for BootstrapTcpClient<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: 'static> BootstrapTcpClient<W> {
    /// Creates a new BootstrapTcpClient
    pub fn new() -> Self {
        Self {
            bootstrap_tcp: BootstrapTcp::new(),
        }
    }

    /// Set ThreadPool for io_group
    pub fn io_group(&mut self, io_group: ThreadPool) -> &mut Self {
        self.bootstrap_tcp.io_group(io_group);
        self
    }

    /// Creates pipeline instances from when calling [BootstrapTcpClient::connect].
    pub fn pipeline(
        &mut self,
        pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>,
    ) -> &mut Self {
        self.bootstrap_tcp.pipeline(pipeline_factory_fn);
        self
    }

    /// Connects to the remote peer
    pub async fn connect<A: AsyncToSocketAddrs>(
        &mut self,
        addr: A,
    ) -> Result<Rc<dyn OutboundPipeline<W>>, Error> {
        self.bootstrap_tcp.connect(addr).await
    }

    /// Gracefully stop the client
    pub async fn stop(&self) {
        self.bootstrap_tcp.stop().await
    }
}
