use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub port: u16,
    pub host: IpAddr,
    pub db_path: PathBuf,
    pub auto_start: bool,
    pub password: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            port: crate::DEFAULT_PORT,
            host: "0.0.0.0".parse().unwrap(),
            db_path: PathBuf::from(crate::DEFAULT_DB_NAME),
            auto_start: true,
            password: String::new(),
        }
    }
}

impl Settings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn local_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// 获取局域网访问 URL（自动检测本机 LAN IP）
    pub fn lan_url(&self) -> String {
        let ip = detect_lan_ip().unwrap_or_else(|| "127.0.0.1".to_string());
        format!("http://{}:{}", ip, self.port)
    }
}

/// 检测局域网 IP，优先返回 192.168.x.x 或 10.x.x.x
/// 过滤掉虚拟网卡（TUN、VMware 等）
fn detect_lan_ip() -> Option<String> {
    let interfaces = get_if_addrs::get_if_addrs().ok()?;

    // 常用局域网前缀，按优先级排序
    let preferred: &[&str] = &["192.168.", "10.", "172."];

    for prefix in preferred {
        for iface in &interfaces {
            if iface.is_loopback() {
                continue;
            }
            let ip = match iface.ip() {
                std::net::IpAddr::V4(v4) => v4.to_string(),
                _ => continue,
            };

            // 跳过常见的虚拟网卡 IP 段
            // TUN 接口通常在 172.18.x.x, VMware 在 192.168.5.x/192.168.150.x
            let skip_prefixes = ["172.18.", "172.17.", "192.168.5.", "192.168.150."];
            if skip_prefixes.iter().any(|p| ip.starts_with(p)) {
                continue;
            }

            if ip.starts_with(prefix) {
                return Some(ip);
            }
        }
    }

    None
}