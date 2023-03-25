use super::*;

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for TCP clients.
pub struct BootstrapTcpClient<W> {
    internal: BootstrapTcp<W>,
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
            internal: BootstrapTcp::new(),
        }
    }

    /// Creates pipeline instances from when calling [BootstrapTcpClient::connect].
    pub fn pipeline(
        &mut self,
        pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>,
    ) -> &mut Self {
        self.internal.pipeline(pipeline_factory_fn);
        self
    }

    /// Connects to the remote peer
    pub async fn connect<A: AsyncToSocketAddrs>(
        &mut self,
        addr: A,
    ) -> Result<Rc<dyn OutboundPipeline<W>>, Error> {
        self.internal.connect(addr).await
    }

    /// Gracefully stop the client
    pub async fn stop(&self) {
        self.internal.stop().await
    }
}
