//! Cross-platform TUN device abstraction
//!
//! Provides a unified interface for TUN devices across Linux and Windows.
//! - Linux: Uses the `tun` crate
//! - Windows: Uses the `wintun` crate

use anyhow::Result;
use std::net::Ipv4Addr;
use tracing::info;

/// TUN device configuration
#[derive(Debug, Clone)]
pub struct TunConfig {
    /// Device name (e.g., "utun0" on macOS, "tun0" on Linux)
    pub name: String,
    /// Device IP address
    pub address: Ipv4Addr,
    /// Netmask
    pub netmask: Ipv4Addr,
    /// MTU (Maximum Transmission Unit)
    pub mtu: u16,
}

impl Default for TunConfig {
    fn default() -> Self {
        Self {
            name: "apfsds0".to_string(),
            address: "10.0.0.1".parse().unwrap(),
            netmask: "255.255.255.0".parse().unwrap(),
            mtu: 1500,
        }
    }
}

// ==================== Linux Implementation ====================
#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use tun::{Device, Configuration};

    pub struct TunDevice {
        device: Device,
    }

    impl TunDevice {
        pub fn create(config: &TunConfig) -> Result<Self> {
            let mut tun_config = Configuration::default();
            tun_config.name(&config.name);
            tun_config.address(config.address);
            tun_config.netmask(config.netmask);
            tun_config.mtu(config.mtu as i32);
            tun_config.up();

            let device = tun::create(&tun_config)?;
            info!("Created TUN device {} on Linux", config.name);

            Ok(Self { device })
        }

        pub fn read(&self, buf: &mut [u8]) -> Result<usize> {
            use std::io::Read;
            let mut device = &self.device;
            Ok(device.read(buf)?)
        }

        pub fn write(&self, buf: &[u8]) -> Result<usize> {
            use std::io::Write;
            let mut device = &self.device;
            Ok(device.write(buf)?)
        }
    }
}

// ==================== Windows Implementation ====================
#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use std::sync::Arc;
    use wintun::{Adapter, Session};

    pub struct TunDevice {
        _adapter: Arc<Adapter>,
        session: Arc<Session>,
    }

    impl TunDevice {
        pub fn create(config: &TunConfig) -> Result<Self> {
            // Load wintun.dll (must be in PATH or current directory)
            let wintun = unsafe { wintun::load()? };
            
            // Create adapter (returns Arc<Adapter>)
            let adapter = Adapter::create(&wintun, &config.name, "APFSDS", None)?;
            
            // Start session and wrap in Arc for API calls
            let session = Arc::new(adapter.start_session(wintun::MAX_RING_CAPACITY)?);
            
            // Set IP address (requires netsh or SetUnicastIpAddressEntry)
            // For now, log a warning that manual config is needed
            tracing::warn!(
                "Windows TUN created. Please configure IP {} via netsh or Windows settings.",
                config.address
            );
            
            info!("Created TUN device {} on Windows", config.name);
            
            Ok(Self {
                _adapter: adapter,
                session,
            })
        }

        pub fn read(&self, buf: &mut [u8]) -> Result<usize> {
            // Use blocking receive (requires Arc<Session>)
            match self.session.receive_blocking() {
                Ok(packet) => {
                    let bytes = packet.bytes();
                    let len = bytes.len().min(buf.len());
                    buf[..len].copy_from_slice(&bytes[..len]);
                    Ok(len)
                }
                Err(e) => Err(anyhow::anyhow!("Wintun read error: {}", e)),
            }
        }

        pub fn write(&self, buf: &[u8]) -> Result<usize> {
            // allocate_send_packet requires Arc<Session>
            let mut packet = self.session.allocate_send_packet(buf.len() as u16)?;
            packet.bytes_mut().copy_from_slice(buf);
            self.session.send_packet(packet);
            Ok(buf.len())
        }
    }
}

// ==================== Stub for other platforms ====================
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
mod platform {
    use super::*;

    pub struct TunDevice;

    impl TunDevice {
        pub fn create(_config: &TunConfig) -> Result<Self> {
            Err(anyhow::anyhow!("TUN devices not supported on this platform"))
        }

        pub fn read(&self, _buf: &mut [u8]) -> Result<usize> {
            unimplemented!()
        }

        pub fn write(&self, _buf: &[u8]) -> Result<usize> {
            unimplemented!()
        }
    }
}

// Re-export platform-specific implementation
pub use platform::TunDevice;
