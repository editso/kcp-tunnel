use clap::Parser;
use kcp_rust::Config;
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

#[tokio::main(flavor = "multi_thread", worker_threads = 6)]
async fn main() -> std::io::Result<()> {
    env_logger::builder()
        .default_format()
        // .filter_module("kcp_rust", log::LevelFilter::Trace)
        .filter_level(log::LevelFilter::Trace)
        .init();

    let args = Args::parse();
    let config = Config::default();

    let works = args.works.min(6);

    let mut kcp_tunnel = kcp_rust::KcpConnector::new::<KcpRuntimeWithTokio>(
        {
            let udp = tokio::net::UdpSocket::bind("0.0.0.0:0").await?;
            udp.connect(args.tunnel).await?;
            kcp_tunnel::TunnelSocket(Arc::new(udp))
        },
        config,
    )?;

    let tcp_listener = tokio::net::TcpListener::bind(args.listen).await?;

    log::info!("work threads: {}", works);
    log::info!("tcp listener started: {}", tcp_listener.local_addr()?);
    log::info!("all connections will be forwarded to {}", args.tunnel);

    loop {
        let (tcp_stream, addr) = tcp_listener.accept().await?;
        let (kcp_conv, kcp_stream) = kcp_tunnel.open().await?;
        let kcp_stream = KcpTunnelStream(kcp_stream);

        tokio::spawn(async move {
            let addr = format!("{}({}) -> {}", args.tunnel, kcp_conv, addr);

            log::debug!("start forward {}", addr);

            let (mut tcp_reader, mut tcp_writer) = tokio::io::split(tcp_stream);
            let (mut kcp_reader, mut kcp_writer) = tokio::io::split(kcp_stream);

            tokio::select!(
                _ = kcp_tunnel::copy(&mut kcp_reader, &mut tcp_writer) => {

                },
                _ = kcp_tunnel::copy(&mut tcp_reader, &mut kcp_writer) => {

                },
            );

            log::debug!("close stream {}", addr);
        });
    }
}
