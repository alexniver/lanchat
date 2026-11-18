//! 单个 Peer 连接的读写逻辑。
//!
//! 每个 TCP 连接启动两个 tokio task：
//! - 读 task：按行读取 → 反序列化 Message → 发送 AppEvent
//! - 写 task：从专属 mpsc channel 接收 → 序列化 → 写入 TCP stream

use std::net::SocketAddr;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::network::AppEvent;
use crate::protocol::{HelloPayload, Message};

const CHANNEL_CAPACITY: usize = 256;

/// 处理入站连接（被动方）：
/// 1. 读取对端 Hello
/// 2. 回复本机 Hello
/// 3. 发送 PeerConnected 事件
/// 4. Split stream 并 spawn 读写 task
pub async fn handle_connection(
    mut stream: TcpStream,
    addr: SocketAddr,
    app_tx: mpsc::Sender<AppEvent>,
    my_node_id: String,
    my_display_name: String,
) -> anyhow::Result<()> {
    tracing::info!("handle_connection (入站): 开始处理连接 {}", addr);

    // 1. 逐字节读取对端 Hello
    let mut buf = Vec::with_capacity(512);
    let mut byte = [0u8; 1];
    let hello_payload = loop {
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
                        Message::Hello(hello) => break hello,
                        other => {
                            return Err(anyhow::anyhow!(
                                "握手时期望 Hello，收到: {:?}",
                                other
                            ));
                        }
                    }
                }
            }
        }
    };

    tracing::info!(
        "入站握手收到 Hello: {} ({})",
        hello_payload.display_name,
        hello_payload.node_id
    );

    // 2. 回复本机 Hello
    let my_hello = Message::Hello(HelloPayload {
        node_id: my_node_id.clone(),
        display_name: my_display_name.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    });
    let mut json = serde_json::to_string(&my_hello)?;
    json.push('\n');
    stream.write_all(json.as_bytes()).await?;

    tracing::info!("入站握手已回复本机 Hello");

    // 3. 创建 channel 并发送 PeerConnected
    let (peer_tx, peer_rx) = mpsc::channel::<Message>(CHANNEL_CAPACITY);

    app_tx
        .send(AppEvent::PeerConnected {
            node_id: hello_payload.node_id.clone(),
            display_name: hello_payload.display_name.clone(),
            addr,
            tx: peer_tx,
        })
        .await?;

    tracing::info!(
        "handle_connection: PeerConnected 已发送 (node_id={})",
        hello_payload.node_id
    );

    // 4. Split 并 spawn 读写 task（握手已完成，直接进入 read_loop）
    let (read_half, write_half) = stream.into_split();

    let read_app_tx = app_tx.clone();
    let read_addr = addr;
    let read_handle = tokio::spawn(async move {
        if let Err(e) = read_loop(read_half, read_addr, read_app_tx).await {
            tracing::warn!("读 task 退出 ({}): {}", read_addr, e);
        }
    });

    let write_addr = addr;
    let write_handle = tokio::spawn(async move {
        if let Err(e) = write_loop(write_half, peer_rx).await {
            tracing::warn!("写 task 退出 ({}): {}", write_addr, e);
        }
    });

    tokio::select! {
        _ = read_handle => {}
        _ = write_handle => {}
    }

    let _ = app_tx
        .send(AppEvent::PeerDisconnected {
            node_id: hello_payload.node_id,
        })
        .await;

    Ok(())
}

/// 处理主动连接（已发送 Hello、已读取对端 Hello）：stream 中后续数据直接进入消息循环。
pub async fn handle_connection_already_hello(
    stream: TcpStream,
    addr: SocketAddr,
    peer_node_id: String,
    peer_rx: mpsc::Receiver<Message>,
    app_tx: mpsc::Sender<AppEvent>,
) -> anyhow::Result<()> {
    let (read_half, write_half) = stream.into_split();

    let read_app_tx = app_tx.clone();
    let read_addr = addr;
    let read_handle = tokio::spawn(async move {
        if let Err(e) = read_loop(read_half, read_addr, read_app_tx).await {
            tracing::warn!("读 task 退出 ({}): {}", read_addr, e);
        }
    });

    let write_addr = addr;
    let write_handle = tokio::spawn(async move {
        if let Err(e) = write_loop(write_half, peer_rx).await {
            tracing::warn!("写 task 退出 ({}): {}", write_addr, e);
        }
    });

    tokio::select! {
        _ = read_handle => {}
        _ = write_handle => {}
    }

    let _ = app_tx
        .send(AppEvent::PeerDisconnected {
            node_id: peer_node_id,
        })
        .await;

    Ok(())
}

/// 读循环（所有消息直接转发为 AppEvent::Message）
async fn read_loop(
    read_half: OwnedReadHalf,
    addr: SocketAddr,
    app_tx: mpsc::Sender<AppEvent>,
) -> anyhow::Result<()> {
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            return Err(anyhow::anyhow!("对端关闭连接"));
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let msg: Message = match serde_json::from_str(trimmed) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("JSON 解析失败 ({}): {} — {}", addr, e, trimmed);
                continue;
            }
        };

        app_tx
            .send(AppEvent::Message { msg, from: addr })
            .await?;
    }
}

/// 写循环：从 mpsc channel 接收 Message → 序列化 → 写入 TCP stream
async fn write_loop(
    mut write_half: OwnedWriteHalf,
    mut rx: mpsc::Receiver<Message>,
) -> anyhow::Result<()> {
    while let Some(msg) = rx.recv().await {
        let mut json = serde_json::to_string(&msg)?;
        json.push('\n');
        write_half.write_all(json.as_bytes()).await?;
    }
    Ok(())
}
