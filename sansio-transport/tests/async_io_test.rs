#[cfg(test)]
mod tests {
    use anyhow::Result;
    use sansio_transport::{
        AsyncUdpSocket, BATCH_SIZE, Capabilities, EcnCodepoint, RecvMeta, Transmit, UdpSocket,
    };
    use std::io::IoSliceMut;
    use std::net::Ipv4Addr;
    use std::time::Instant;

    #[tokio::test]
    async fn test_ecn() -> Result<()> {
        env_logger::init();
        let capabilities = Capabilities::new();
        let socket1 = UdpSocket::bind("127.0.0.1:0").await?;
        let socket2 = UdpSocket::bind("127.0.0.1:0").await?;
        let addr2 = socket2.local_addr()?;

        let mut transmits = Vec::with_capacity(1);
        for i in 0..1 {
            let contents = (i as u64).to_be_bytes().to_vec();
            transmits.push(Transmit {
                destination: addr2,
                ecn: Some(EcnCodepoint::Ce),
                segment_size: None,
                contents,
                src_ip: Some(Ipv4Addr::LOCALHOST.into()),
            });
        }

        let (tx, mut rx) = tokio::sync::mpsc::channel(1);

        tokio::spawn(async move {
            let mut storage = [[0u8; 1200]; BATCH_SIZE];
            let mut buffers = Vec::with_capacity(BATCH_SIZE);
            let mut rest = &mut storage[..];
            for _ in 0..BATCH_SIZE {
                let (b, r) = rest.split_at_mut(1);
                rest = r;
                buffers.push(IoSliceMut::new(&mut b[0]));
            }

            let mut meta = [RecvMeta::default(); BATCH_SIZE];
            let n = socket2.recv(&mut buffers, &mut meta).await.unwrap();
            for i in 0..n {
                println!(
                    "received {} {:?} {:?}",
                    i,
                    &buffers[i][..meta[i].len],
                    &meta[i]
                );
            }
            let _ = tx.send(meta[0].ecn).await;
        });

        let start = Instant::now();

        log::debug!("before send");
        socket1.send(&capabilities, &transmits).await?;
        log::debug!("after send");

        println!("sent {} packets in {}ms", 1, start.elapsed().as_millis());

        let ecn = rx.recv().await.unwrap();

        #[cfg(not(windows))]
        {
            assert!(ecn.is_some());
            assert_eq!(EcnCodepoint::Ce, ecn.unwrap());
        }

        Ok(())
    }
}
