//! mDNS 广播与发现。
//!
//! 在一个独立的标准库线程中运行：
//! 1. 注册本机服务 `_lanchat._tcp.local.`
//! 2. 监听其他 `_lanchat._tcp.local.` 服务的出现/消失
//! 3. 通过 `mpsc::Sender<AppEvent>` 将事件发送给 tokio 运行时

use std::net::SocketAddr;
use std::thread;
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::sync::mpsc as tokio_mpsc;

use crate::network::AppEvent;

const SERVICE_TYPE: &str = "_lanchat._tcp.local.";

/// 启动 mDNS 发现线程。
///
/// 返回 `ServiceDaemon` 的句柄（用于程序退出时取消注册）。
pub fn start(
    node_id: String,
    display_name: String,
    port: u16,
    app_tx: tokio_mpsc::Sender<AppEvent>,
) -> anyhow::Result<ServiceDaemon> {
    tracing::info!("启动 mDNS 发现，服务类型: {}", SERVICE_TYPE);

    let daemon = ServiceDaemon::new()?;
    let daemon_clone = daemon.clone();

    thread::Builder::new()
        .name("mdns-discovery".to_string())
        .spawn(move || {
            discovery_thread(daemon_clone, node_id, display_name, port, app_tx);
        })?;

    Ok(daemon)
}

fn discovery_thread(
    daemon: ServiceDaemon,
    node_id: String,
    display_name: String,
    port: u16,
    app_tx: tokio_mpsc::Sender<AppEvent>,
) {
    // 1. 注册本机服务
    // 0.19 API: my_name 只是实例名（node_id），不含服务类型后缀
    let instance_name = format!("{}._lanchat._tcp.local.", node_id);

    let mut txt_props = std::collections::HashMap::new();
    txt_props.insert("node_id".to_string(), node_id.clone());
    txt_props.insert("name".to_string(), display_name.clone());

    let host_ip = get_local_ip();

    let service_info = ServiceInfo::new(
        SERVICE_TYPE,
        &node_id, // 0.19: my_name 是纯粹的实例名
        &format!("{}.local.", hostname_for_mdns()),
        host_ip.to_string().as_str(),
        port,
        txt_props,
    )
    .unwrap_or_else(|e| {
        tracing::error!("创建 ServiceInfo 失败: {}", e);
        panic!("mDNS ServiceInfo 创建失败: {}", e);
    });

    if let Err(e) = daemon.register(service_info) {
        tracing::error!("mDNS 注册服务失败: {}", e);
        return;
    }
    tracing::info!(
        "mDNS 服务已注册: {} @ {}:{}",
        instance_name,
        host_ip,
        port
    );

    // 2. 启动 browse
    let receiver = match daemon.browse(SERVICE_TYPE) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("mDNS browse 失败: {}", e);
            return;
        }
    };

    // 3. 内部 tokio runtime，用于发送 async 事件
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            tracing::error!("创建 mDNS 内部 tokio runtime 失败: {}", e);
            return;
        }
    };

    loop {
        match receiver.recv_timeout(Duration::from_secs(1)) {
            Ok(event) => {
                rt.block_on(async {
                    handle_mdns_event(&app_tx, &event, host_ip, port, &node_id).await;
                });
            }
            Err(err) => {
                // flume::RecvTimeoutError — 检查是否 Disconnected
                let is_disconnected = format!("{}", err).contains("Disconnected");
                if is_disconnected {
                    tracing::error!("mDNS browse receiver 断开: {}", err);
                    break;
                }
                // Timeout: 继续循环
            }
        }
    }
}

async fn handle_mdns_event(
    app_tx: &tokio_mpsc::Sender<AppEvent>,
    event: &ServiceEvent,
    my_ip: std::net::Ipv4Addr,
    my_port: u16,
    my_node_id: &str,
) {
    tracing::debug!("mDNS 事件: {:?}", event);
    match event {
        ServiceEvent::ServiceResolved(info) => {
            tracing::info!(
                "mDNS ServiceResolved: {} @ {:?}:{}",
                info.get_fullname(),
                info.get_addresses(),
                info.get_port()
            );

            // 忽略自身：IP 和端口同时匹配时跳过
            let port = info.get_port();
            let addresses = info.get_addresses();
            let is_self = port == my_port
                && addresses
                    .iter()
                    .any(|scoped| scoped.to_ip_addr() == std::net::IpAddr::V4(my_ip));
            if is_self {
                tracing::debug!("忽略自身服务 ({}:{})", my_ip, my_port);
                return;
            }

            // 0.19: get_property 返回 TxtProperty，其 Display 输出 "key=value" 格式
            // 改用 get_property_val_str 获取纯 value
            let their_node_id = match info.get_property_val_str("node_id") {
                Some(val) => val.to_string(),
                None => return,
            };

            let display_name = info
                .get_property_val_str("name")
                .map(|v| v.to_string())
                .unwrap_or_else(|| "unknown".to_string());

            // addresses 和 port 已在自身过滤中获取，直接复用
            let addr = addresses.iter().next().map(|scoped| scoped.to_ip_addr());

            let Some(ip) = addr else {
                tracing::warn!("发现节点 {} 但没有地址信息", their_node_id);
                return;
            };

            let socket_addr = SocketAddr::new(ip, port);

            tracing::info!(
                "发现新节点: {} ({}) @ {}",
                display_name,
                their_node_id,
                socket_addr
            );

            if let Err(e) = app_tx
                .send(AppEvent::PeerDiscovered {
                    node_id: their_node_id,
                    display_name,
                    addr: socket_addr,
                })
                .await
            {
                tracing::error!("发送 PeerDiscovered 事件失败: {}", e);
            } else {
                tracing::info!("已发送 PeerDiscovered 事件到 channel");
            }
        }
        ServiceEvent::ServiceFound(_service_type, fullname) => {
            tracing::info!("mDNS ServiceFound: {}", fullname);
            // fullname 格式: "node_id._lanchat._tcp.local."
            // 从中提取 node_id 并记录
            if let Some(node_id) = fullname
                .strip_suffix(&format!(".{}", SERVICE_TYPE))
                .or_else(|| fullname.strip_suffix("._lanchat._tcp.local."))
            {
                let nid = node_id.trim_end_matches('.');
                if nid != my_node_id {
                    tracing::info!(
                        "mDNS ServiceFound 发现新节点: node_id={} (等待 ServiceResolved 获取地址)",
                        nid
                    );
                }
            }
        }
        ServiceEvent::ServiceRemoved(instance_name, _service_type) => {
            if let Some(node_id) = instance_name
                .strip_suffix(&format!(".{}", SERVICE_TYPE))
                .or_else(|| instance_name.strip_suffix("._lanchat._tcp.local."))
            {
                let node_id = node_id.trim_end_matches('.');
                if node_id != my_node_id {
                    tracing::info!("节点已离开: {}", node_id);
                    let _ = app_tx
                        .send(AppEvent::PeerVanished {
                            node_id: node_id.to_string(),
                        })
                        .await;
                }
            }
        }
        ServiceEvent::SearchStarted(ty) => {
            tracing::info!("mDNS SearchStarted: {}", ty);
        }
        ServiceEvent::SearchStopped(ty) => {
            tracing::info!("mDNS SearchStopped: {}", ty);
        }
        _ => {}
    }
}

/// 获取本机首选非回环 IPv4 地址
fn get_local_ip() -> std::net::Ipv4Addr {
    use std::net::UdpSocket;
    match UdpSocket::bind("0.0.0.0:0") {
        Ok(socket) => {
            if socket.connect("8.8.8.8:53").is_ok() {
                if let Ok(local) = socket.local_addr() {
                    match local.ip() {
                        std::net::IpAddr::V4(ip) => {
                            if !ip.is_loopback() {
                                return ip;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        _ => {}
    }
    std::net::Ipv4Addr::new(127, 0, 0, 1)
}

fn hostname_for_mdns() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "lanchat".to_string())
}
