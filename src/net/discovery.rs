use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::Result;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::sync::mpsc;

const SERVICE_TYPE: &str = "_lanchat._tcp.local.";

#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    Resolved {
        node_id: String,
        name: String,
        addrs: Vec<IpAddr>,
        port: u16,
    },
    Removed {
        node_id: String,
    },
    Error(String),
}

pub struct Discovery {
    daemon: ServiceDaemon,
    thread: JoinHandle<()>,
}

impl Drop for Discovery {
    fn drop(&mut self) {
        let _ = self.daemon.shutdown();
        // The browse receiver closes during shutdown; the thread exits on timeout/disconnect.
        let _ = self.thread.is_finished();
    }
}

pub fn start(
    node_id: String,
    name: String,
    port: u16,
    events: mpsc::Sender<DiscoveryEvent>,
) -> Result<Discovery> {
    let daemon = ServiceDaemon::new()?;
    let thread_daemon = daemon.clone();
    let thread = thread::Builder::new()
        .name("lanchat-mdns".to_string())
        .spawn(move || run(thread_daemon, node_id, name, port, events))?;
    Ok(Discovery { daemon, thread })
}

fn run(
    daemon: ServiceDaemon,
    node_id: String,
    name: String,
    port: u16,
    events: mpsc::Sender<DiscoveryEvent>,
) {
    let instance = node_id.clone();
    let host = format!("{}.local.", hostname_for_mdns());
    let ip = get_local_ip();
    let mut props = std::collections::HashMap::new();
    props.insert("id".to_string(), node_id.clone());
    props.insert("name".to_string(), name);
    props.insert("v".to_string(), "1".to_string());
    props.insert("proto".to_string(), "1".to_string());

    let service = match ServiceInfo::new(
        SERVICE_TYPE,
        &instance,
        &host,
        ip.to_string().as_str(),
        port,
        props,
    ) {
        Ok(service) => service,
        Err(error) => {
            send(
                &events,
                DiscoveryEvent::Error(format!("mDNS 服务信息创建失败: {error}")),
            );
            return;
        }
    };
    if let Err(error) = daemon.register(service) {
        send(
            &events,
            DiscoveryEvent::Error(format!("mDNS 注册失败: {error}")),
        );
        return;
    }

    let receiver = match daemon.browse(SERVICE_TYPE) {
        Ok(receiver) => receiver,
        Err(error) => {
            send(
                &events,
                DiscoveryEvent::Error(format!("mDNS browse 失败: {error}")),
            );
            return;
        }
    };

    let my_node_id = node_id;
    loop {
        match receiver.recv_timeout(Duration::from_secs(1)) {
            Ok(event) => match event {
                ServiceEvent::ServiceResolved(info) => {
                    let Some(peer_id) = info.get_property_val_str("id") else {
                        continue;
                    };
                    if peer_id == my_node_id {
                        continue;
                    }
                    let peer_name = info
                        .get_property_val_str("name")
                        .unwrap_or("unknown")
                        .to_string();
                    let port = info.get_port();
                    let addrs: Vec<IpAddr> = info
                        .get_addresses()
                        .iter()
                        .map(|scoped| scoped.to_ip_addr())
                        .collect();
                    if addrs.is_empty() {
                        continue;
                    }
                    if !send(
                        &events,
                        DiscoveryEvent::Resolved {
                            node_id: peer_id.to_string(),
                            name: peer_name,
                            addrs,
                            port,
                        },
                    ) {
                        break;
                    }
                }
                ServiceEvent::ServiceRemoved(_, fullname) => {
                    if let Some(peer_id) = parse_instance_id(&fullname) {
                        if peer_id == my_node_id {
                            continue;
                        }
                        if !send(
                            &events,
                            DiscoveryEvent::Removed {
                                node_id: peer_id.to_string(),
                            },
                        ) {
                            break;
                        }
                    }
                }
                _ => {}
            },
            Err(error) => {
                if format!("{error}").contains("Disconnected") {
                    break;
                }
            }
        }
    }
}

fn send(events: &mpsc::Sender<DiscoveryEvent>, event: DiscoveryEvent) -> bool {
    events.blocking_send(event).is_ok()
}

fn parse_instance_id(fullname: &str) -> Option<&str> {
    fullname
        .strip_suffix(&format!(".{SERVICE_TYPE}"))
        .or_else(|| fullname.strip_suffix("._lanchat._tcp.local."))
        .map(|value| value.trim_end_matches('.'))
        .filter(|value| !value.is_empty())
}

fn hostname_for_mdns() -> String {
    hostname::get()
        .ok()
        .and_then(|host| host.into_string().ok())
        .unwrap_or_else(|| "lanchat".to_string())
}

fn get_local_ip() -> Ipv4Addr {
    if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:53").is_ok() {
            if let Ok(local) = socket.local_addr() {
                if let IpAddr::V4(ip) = local.ip() {
                    if !ip.is_loopback() {
                        return ip;
                    }
                }
            }
        }
    }
    Ipv4Addr::LOCALHOST
}

pub fn socket_addrs(addrs: &[IpAddr], port: u16) -> Vec<SocketAddr> {
    let mut result: Vec<SocketAddr> = addrs
        .iter()
        .map(|addr| SocketAddr::new(*addr, port))
        .collect();
    result.sort_by_key(|addr| match addr {
        SocketAddr::V4(_) => 0,
        SocketAddr::V6(_) => 1,
    });
    result
}
