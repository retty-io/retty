use std::{
    io,
    net::{SocketAddr, ToSocketAddrs},
};

//pub(crate) mod bootstrap_tcp_client;
//pub(crate) mod bootstrap_tcp_server;
pub(crate) mod bootstrap_udp_client;
pub(crate) mod bootstrap_udp_server;
//pub(crate) mod bootstrap_udp_ecn_client;
//pub(crate) mod bootstrap_udp_ecn_server;

fn each_addr<A: ToSocketAddrs, F, T>(addr: A, mut f: F) -> io::Result<T>
where
    F: FnMut(&SocketAddr) -> io::Result<T>,
{
    let addrs = match addr.to_socket_addrs() {
        Ok(addrs) => addrs,
        Err(e) => return Err(e),
    };
    let mut last_err = None;
    for addr in addrs {
        match f(&addr) {
            Ok(l) => return Ok(l),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "could not resolve to any addresses",
        )
    }))
}
