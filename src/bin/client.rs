use clap::Parser;
use std::{net::SocketAddr, sync::Arc};

use kcp_tunnel::{KcpRuntimeWithTokio, KcpTunnelStream};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    // local listening address
    listen: SocketAddr,
    // tunnel listen address
    tunnel: SocketAddr,
    // work thread number
    #[arg(short, default_value_t = 6)]
    works: usize,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::builder()
        .default_format()
        // .filter_module("kcp_rust", log::LevelFilter::Info)
        .filter_module("kcp_tunnel", log::LevelFilter::Trace)
        .filter_level(log::LevelFilter::Info)
        .init();

    let args = Args::parse();

    let works = args.works.min(6);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(works)
        .event_interval(10)
        .enable_all()
        .build()?;

    let mut kcp_tunnel = kcp_rust::KcpConnector::new::<KcpRuntimeWithTokio>({
        let udp = tokio::net::UdpSocket::bind("0.0.0.0:0").await?;
        udp.connect(args.tunnel).await?;
        kcp_tunnel::TunnelSocket(Arc::new(udp))
    })?;

    let tcp_listener = tokio::net::TcpListener::bind(args.listen).await?;

    log::info!("work threads: {}", works);
    log::info!("tcp listener started: {}", tcp_listener.local_addr()?);
    log::info!("all connections will be forwarded to {}", args.tunnel);

    loop {
        let (tcp_stream, addr) = tcp_listener.accept().await?;
        let kcp_stream = KcpTunnelStream(kcp_tunnel.open().await?);

        runtime.spawn(async move {
            log::debug!("start forward: {} -> {}", addr, args.tunnel);

            let (mut kcp_reader, mut kcp_writer) = tokio::io::split(kcp_stream);
            let (mut tcp_reader, mut tcp_writer) = tokio::io::split(tcp_stream);

            tokio::join!(
                tokio::io::copy(&mut kcp_reader, &mut tcp_writer),
                tokio::io::copy(&mut tcp_reader, &mut kcp_writer),
            )
        });
    }
}
