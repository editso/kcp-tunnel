use clap::Parser;
use std::{net::SocketAddr, sync::Arc};

use kcp_tunnel::KcpRuntimeWithTokio;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    // tunnel listen address
    tunnel: SocketAddr,
    // forward to address
    to: SocketAddr,
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

    let kcp_listener = kcp_rust::KcpListener::new::<KcpRuntimeWithTokio>({
        let udp = tokio::net::UdpSocket::bind(args.tunnel).await?;
        kcp_tunnel::TunnelSocket(Arc::new(udp))
    })?;

    log::info!("work threads: {}", works);
    log::info!("kcp listener started: {}", args.tunnel);
    log::info!("all connections will be forwarded to {}", args.to);

    loop {
        let kcp_stream = kcp_tunnel::KcpTunnelStream(kcp_listener.accept().await?);
        let tcp_stream = tokio::net::TcpStream::connect(args.to).await?;

        runtime.spawn(async move {
            log::debug!("start forward {} -> {}", args.tunnel, args.to);

            let (mut kcp_reader, mut kcp_writer) = tokio::io::split(kcp_stream);
            let (mut tcp_reader, mut tcp_writer) = tokio::io::split(tcp_stream);

            tokio::join!(
                tokio::io::copy(&mut kcp_reader, &mut tcp_writer),
                tokio::io::copy(&mut tcp_reader, &mut kcp_writer),
            )
        });
    }
}
