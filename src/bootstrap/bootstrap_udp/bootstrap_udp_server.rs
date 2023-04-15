use super::*;

/// A Bootstrap that makes it easy to bootstrap a pipeline to use for UDP servers.
pub struct BootstrapUdpServer<W> {
    bootstrap_udp: BootstrapUdp<W>,
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
            bootstrap_udp: BootstrapUdp::new(),
        }
    }

    /// Creates pipeline instances from when calling [BootstrapUdpServer::bind].
    pub fn pipeline(
        &mut self,
        pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>,
    ) -> &mut Self {
        self.bootstrap_udp.pipeline(pipeline_factory_fn);
        self
    }

    /// Binds local address and port
    pub async fn bind<A: AsyncToSocketAddrs>(&mut self, addr: A) -> Result<SocketAddr, Error> {
        let local_addr = self.bootstrap_udp.bind(addr).await?;
        let peer_addr: Option<SocketAddr> = None;
        self.bootstrap_udp.connect(peer_addr).await?;
        Ok(local_addr)
    }

    /// Gracefully stop the BootstrapUdpServer
    pub async fn stop(&self) {
        self.bootstrap_udp.stop().await
    }
}
