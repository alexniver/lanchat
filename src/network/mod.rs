pub mod connect;
pub mod discovery;
pub mod peer;
pub mod server;

use std::net::SocketAddr;

use crate::protocol::Message;

/// 应用程序内部事件：网络层与 Event Handler 之间的通信协议
///
/// 区别于 `Message`（网络线上协议），`AppEvent` 承载的是
/// 已解析的、需要修改 `AppState` 的内部通知。
#[derive(Debug)]
pub enum AppEvent {
    /// 从某个 peer 连接收到一条网络消息
    Message {
        msg: Message,
        /// 发送方的 socket 地址，用于关联到 peer
        from: SocketAddr,
    },
    /// mDNS 发现新节点，需要发起 TCP 连接
    PeerDiscovered {
        node_id: String,
        display_name: String,
        addr: SocketAddr,
    },
    /// mDNS 检测到节点消失
    PeerVanished {
        node_id: String,
    },
    /// TCP 连接已建立且 Hello 握手完成
    PeerConnected {
        node_id: String,
        display_name: String,
        addr: SocketAddr,
        /// 向该 peer 发送消息的通道（由 peer 模块创建后传回）
        tx: tokio::sync::mpsc::Sender<Message>,
    },
    /// Peer 连接断开（读/写 task 退出）
    PeerDisconnected {
        node_id: String,
    },
    /// 用户通过 TUI 输入框发送聊天消息
    SendChat {
        content: String,
    },
}
