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

    /// Creates pipeline instances from when calling [BootstrapUdpClient::connect].
    pub fn pipeline(
        &mut self,
        pipeline_factory_fn: PipelineFactoryFn<TaggedBytesMut, W>,
    ) -> &mut Self {
        self.internal.pipeline(pipeline_factory_fn);
        self
    }

    /// Binds local address and port, return local socket address
    pub fn bind<A: ToString>(&mut self, addr: A) -> Result<SocketAddr, Error> {
        self.internal.bind(addr)
    }

    /// Connects to the remote peer
    pub async fn connect(
        &mut self,
        _addr: SocketAddr,
    ) -> Result<Rc<dyn OutboundPipeline<W>>, Error> {
        //TODO:
        /*let socket = Rc::clone(self.internal.socket.as_ref().ok_or(Error::new(
            ErrorKind::AddrNotAvailable,
            "socket is not bind yet",
        ))?);

        socket.connect(addr).await?;
         */

        let pipeline_wr = self.internal.serve()?;

        Ok(pipeline_wr)
    }

    /// Gracefully stop the client
    pub async fn stop(&self) {
        self.internal.stop().await;
    }
}
