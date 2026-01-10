//! Emergency mode checker using crates.io API

use crate::config::EmergencyConfig;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

/// Global emergency mode flag
static EMERGENCY_MODE: AtomicBool = AtomicBool::new(false);

/// Check if emergency mode is active
pub fn is_emergency_mode() -> bool {
    EMERGENCY_MODE.load(Ordering::Relaxed)
}

/// Trigger emergency mode
pub fn trigger_emergency() {
    EMERGENCY_MODE.store(true, Ordering::SeqCst);
    warn!("🚨 EMERGENCY MODE ACTIVATED 🚨");
}

/// Start the emergency mode checker
pub fn start_checker(config: EmergencyConfig) -> JoinHandle<()> {
    tokio::spawn(async move {
        if !config.enabled {
            info!("Emergency mode checker disabled");
            return;
        }

        info!(
            "Emergency mode checker started, checking '{}' every {}s",
            config.crate_name, config.check_interval
        );

        let client = crates_io_api::AsyncClient::new(
            "apfsds-client (https://github.com/rand0mdevel0per/apfsds.rs)",
            Duration::from_millis(1000),
        );

        match client {
            Ok(client) => {
                loop {
                    tokio::time::sleep(Duration::from_secs(config.check_interval)).await;

                    match check_crate_status(&client, &config.crate_name).await {
                        Ok(yanked) => {
                            if yanked {
                                trigger_emergency();
                                // Add random delay before actually stopping (0-1 hour)
                                let delay = fastrand::u64(0..3600);
                                info!("Will shutdown in {} seconds", delay);
                                tokio::time::sleep(Duration::from_secs(delay)).await;
                                std::process::exit(0);
                            }
                        }
                        Err(e) => {
                            // Log but don't panic - network issues shouldn't stop us
                            error!("Failed to check crate status: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to create crates.io client: {}", e);
            }
        }
    })
}

/// Check if the crate's latest version is yanked
async fn check_crate_status(
    client: &crates_io_api::AsyncClient,
    crate_name: &str,
) -> Result<bool, crates_io_api::Error> {
    let crate_info = client.get_crate(crate_name).await?;

    // Check if the latest version is yanked
    if let Some(version) = crate_info.versions.first() {
        Ok(version.yanked)
    } else {
        // No versions = treat as emergency (crate deleted?)
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emergency_mode_flag() {
        assert!(!is_emergency_mode());

        trigger_emergency();
        assert!(is_emergency_mode());

        // Reset for other tests
        EMERGENCY_MODE.store(false, Ordering::SeqCst);
    }
}
