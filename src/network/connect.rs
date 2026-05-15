//! 主动连接逻辑。
//!
//! 当 mDNS 发现新节点时：
//! 1. node_id 字典序较小的发起连接（避免双向互连）
//! 2. TCP 连接 → 发送本机 Hello → 读取对端 Hello
//! 3. 将已握手的 stream 交给 peer 模块

use std::net::SocketAddr;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::network::peer;
use crate::network::AppEvent;
use crate::protocol::Message;

/// 决定本机是否应主动发起连接。
///
/// 使用端口号比较作为去重依据：端口号较小的一方发起连接。
/// 这替代了原本的 node_id 字典序比较，避免两台机器使用
/// 相同 node_id 时双方都不发起连接。
pub fn should_connect(my_port: u16, their_port: u16) -> bool {
    let result = my_port < their_port;
    tracing::debug!(
        "should_connect: my_port={} their_port={} result={}",
        my_port,
        their_port,
        result
    );
    result
}

pub async fn connect_and_handshake(
    addr: SocketAddr,
    my_node_id: String,
    my_display_name: String,
    app_tx: mpsc::Sender<AppEvent>,
) -> anyhow::Result<()> {
    tracing::info!(
        "connect_and_handshake: 正在连接 {} (my_node_id={})",
        addr,
        my_node_id
    );

    let mut stream = TcpStream::connect(addr).await?;
    tracing::info!("TCP 连接已建立: {}", addr);

    // 1. 发送 Hello
    let hello = Message::Hello(crate::protocol::HelloPayload {
        node_id: my_node_id.clone(),
        display_name: my_display_name.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    });
    let mut json = serde_json::to_string(&hello)?;
    json.push('\n');
    stream.write_all(json.as_bytes()).await?;

    // 2. 逐字节读取对端 Hello（避免 BufReader 内部缓冲问题）
    let mut buf = Vec::with_capacity(512);
    let mut byte = [0u8; 1];
    loop {
        match stream.read(&mut byte).await? {
            0 => return Err(anyhow::anyhow!("对端在握手期间关闭连接")),
            _ => {
                buf.push(byte[0]);
                if byte[0] == b'\n' {
                    let line = String::from_utf8_lossy(&buf);
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        buf.clear();
                        continue;
                    }
                    let msg: Message = serde_json::from_str(trimmed)?;
                    match msg {
                        Message::Hello(hello) => {
                            tracing::info!(
                                "主动连接握手完成: {} ({}) @ {}",
                                hello.display_name,
                                hello.node_id,
                                addr
                            );

                            let (peer_tx, peer_rx) = mpsc::channel::<Message>(256);

                            tracing::info!(
                                "connect_and_handshake: 准备发送 PeerConnected (node_id={})",
                                hello.node_id
                            );

                            // 通知 Event Handler
                            app_tx
                                .send(AppEvent::PeerConnected {
                                    node_id: hello.node_id.clone(),
                                    display_name: hello.display_name.clone(),
                                    addr,
                                    tx: peer_tx,
                                })
                                .await?;

                            tracing::info!(
                                "connect_and_handshake: PeerConnected 已发送 (node_id={})",
                                hello.node_id
                            );

                            app_tx
                                .send(AppEvent::Message {
                                    msg: Message::Hello(hello),
                                    from: addr,
                                })
                                .await?;

                            // 3. stream 完整可用，交给 peer 模块
                            tokio::spawn(async move {
                                if let Err(e) =
                                    peer::handle_connection_already_hello(
                                        stream,
                                        addr,
                                        peer_rx,
                                        app_tx,
                                    )
                                    .await
                                {
                                    tracing::warn!(
                                        "主动连接 peer handler 退出 ({}): {}",
                                        addr,
                                        e
                                    );
                                }
                            });

                            return Ok(());
                        }
                        other => {
                            return Err(anyhow::anyhow!(
                                "握手期间期望 Hello，收到: {:?}",
                                other
                            ));
                        }
                    }
                }
            }
        }
    }
}
