//! TCP Server：监听随机端口，接受入站连接，为每个连接 spawn 读写 task。

use std::net::SocketAddr;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::network::peer;
use crate::network::AppEvent;

/// 启动 TCP Server，绑定随机端口。
///
/// 返回实际绑定的端口号，供 mDNS 广播使用。
pub async fn start(
    app_tx: mpsc::Sender<AppEvent>,
    my_node_id: String,
    my_display_name: String,
) -> anyhow::Result<u16> {
    let listener = TcpListener::bind("0.0.0.0:0").await?;
    let local_addr = listener.local_addr()?;
    let port = local_addr.port();

    tracing::info!("TCP Server 已启动，监听端口: {}", port);

    // Spawn accept 循环
    tokio::spawn(accept_loop(listener, app_tx, my_node_id, my_display_name));

    Ok(port)
}

async fn accept_loop(
    listener: TcpListener,
    app_tx: mpsc::Sender<AppEvent>,
    my_node_id: String,
    my_display_name: String,
) {
    tracing::info!("accept_loop 已启动");
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                tracing::info!("accept_loop: 接受新入站连接: {}", addr);
                tokio::spawn(handle_inbound(
                    stream,
                    addr,
                    app_tx.clone(),
                    my_node_id.clone(),
                    my_display_name.clone(),
                ));
            }
            Err(e) => {
                tracing::error!("accept 失败: {}", e);
            }
        }
    }
}

async fn handle_inbound(
    stream: TcpStream,
    addr: SocketAddr,
    app_tx: mpsc::Sender<AppEvent>,
    my_node_id: String,
    my_display_name: String,
) {
    if let Err(e) =
        peer::handle_connection(stream, addr, app_tx, my_node_id, my_display_name).await
    {
        tracing::error!("入站连接 {} 处理错误: {}", addr, e);
    }
}
