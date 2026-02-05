//! APFSDS Client Library

pub mod config;
pub mod doh;
pub mod emergency;
pub mod local_dns;
pub mod socks5;
pub mod wss;
pub mod tun_device;
pub mod mobile;

use anyhow::Result;

pub async fn run_tun(config: &config::ClientConfig) -> Result<()> {
    tracing::info!("Initializing TUN device...");
    
    // Parse CIDR "10.0.0.2/24" -> (Ip, Netmask)
    // Simple helper or dumb logic for now
    let (addr, netmask) = parse_cidr(&config.tun.address).unwrap_or((
        "10.0.0.2".parse().unwrap(),
        "255.255.255.0".parse().unwrap()
    ));
    
    let tun_config = tun_device::TunConfig {
        name: config.tun.device.clone(),
        address: addr,
        netmask: netmask,
        mtu: config.tun.mtu,
    };
    
    let mut device = tun_device::TunDevice::create(&tun_config)?;
    
    tracing::info!("TUN device started. Reading packets...");
    
    // Simple echo loop stub
    let mut buf = [0u8; 1504];
    loop {
        match device.read(&mut buf) {
            Ok(n) => {
                tracing::debug!("Read {} bytes from TUN", n);
            }
            Err(e) => {
                tracing::error!("TUN read error: {}", e);
                break;
            }
        }
    }
    
    Ok(())
}

fn parse_cidr(cidr: &str) -> Option<(std::net::Ipv4Addr, std::net::Ipv4Addr)> {
    let parts: Vec<&str> = cidr.split('/').collect();
    if parts.len() != 2 { return None; }
    
    let ip = parts[0].parse().ok()?;
    let bits: u32 = parts[1].parse().ok()?;
    
    let mask: u32 = !((1u32 << (32 - bits)) - 1);
    let mask_ip = std::net::Ipv4Addr::from(mask);
    
    Some((ip, mask_ip))
}
