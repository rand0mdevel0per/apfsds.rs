//! Geo-IP based routing for Exit Nodes
//!
//! Uses MaxMind GeoLite2 database to determine client location
//! and select the optimal exit node based on geographic proximity.

use anyhow::{Result, anyhow};
use maxminddb::{geoip2, Reader};
use std::net::IpAddr;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, warn};

/// Geographic location data
#[derive(Debug, Clone)]
pub struct GeoLocation {
    pub country_code: Option<String>,
    pub city: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
}

impl Default for GeoLocation {
    fn default() -> Self {
        Self {
            country_code: None,
            city: None,
            latitude: 0.0,
            longitude: 0.0,
        }
    }
}

/// Geo-IP resolver using MaxMind database
pub struct GeoIPResolver {
    reader: Reader<Vec<u8>>,
}

impl GeoIPResolver {
    /// Create a new resolver from database file path
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let reader = Reader::open_readfile(db_path.as_ref())?;
        Ok(Self { reader })
    }
    
    /// Create from embedded database bytes
    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        let reader = Reader::from_source(data)?;
        Ok(Self { reader })
    }
    
    /// Lookup IP address location
    pub fn lookup(&self, ip: IpAddr) -> Option<GeoLocation> {
        let result = match self.reader.lookup(ip) {
            Ok(r) => r,
            Err(e) => {
                debug!("GeoIP lookup failed for {}: {}", ip, e);
                return None;
            }
        };
        
        let city: geoip2::City = match result.decode() {
            Ok(Some(c)) => c,
            Ok(None) => return None,
            Err(e) => {
                debug!("GeoIP decode failed for {}: {}", ip, e);
                return None;
            }
        };
        
        // Access fields directly (maxminddb 0.27 API)
        let latitude = city.location.latitude.unwrap_or(0.0);
        let longitude = city.location.longitude.unwrap_or(0.0);
        
        Some(GeoLocation {
            country_code: city.country.iso_code.map(|s| s.to_string()),
            city: city.city.names.english.map(|s| s.to_string()),
            latitude,
            longitude,
        })
    }
}

/// Calculate Haversine distance between two points (in km)
pub fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6371.0; // Earth radius in km
    
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    
    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    
    let a = (d_lat / 2.0).sin().powi(2) 
        + lat1_rad.cos() * lat2_rad.cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().asin();
    
    R * c
}

/// Exit node with location info
#[derive(Debug, Clone)]
pub struct GeoExitNode {
    pub name: String,
    pub endpoint: String,
    pub weight: f64,
    pub latitude: f64,
    pub longitude: f64,
}

impl GeoExitNode {
    /// Calculate score based on distance (lower = better)
    pub fn score(&self, client_geo: &GeoLocation) -> f64 {
        let distance = haversine_distance(
            client_geo.latitude,
            client_geo.longitude,
            self.latitude,
            self.longitude,
        );
        
        // Apply weight (higher weight = lower score)
        distance / self.weight
    }
}

/// Select the best exit node for a client
pub fn select_best_exit<'a>(
    nodes: &'a [GeoExitNode],
    client_geo: &GeoLocation,
) -> Option<&'a GeoExitNode> {
    if nodes.is_empty() {
        return None;
    }
    
    // If client location unknown, return highest weight node
    if client_geo.latitude == 0.0 && client_geo.longitude == 0.0 {
        return nodes.iter().max_by(|a, b| 
            a.weight.partial_cmp(&b.weight).unwrap_or(std::cmp::Ordering::Equal)
        );
    }
    
    nodes.iter()
        .min_by(|a, b| {
            let score_a = a.score(client_geo);
            let score_b = b.score(client_geo);
            score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_haversine_distance() {
        // Tokyo to Singapore
        let distance = haversine_distance(35.6762, 139.6503, 1.3521, 103.8198);
        assert!((distance - 5300.0).abs() < 100.0); // ~5300km
    }
    
    #[test]
    fn test_select_best_exit() {
        let nodes = vec![
            GeoExitNode {
                name: "tokyo".to_string(),
                endpoint: "10.0.1.100:25347".to_string(),
                weight: 1.0,
                latitude: 35.6762,
                longitude: 139.6503,
            },
            GeoExitNode {
                name: "singapore".to_string(),
                endpoint: "10.0.1.101:25347".to_string(),
                weight: 1.0,
                latitude: 1.3521,
                longitude: 103.8198,
            },
        ];
        
        // Client in Shanghai
        let client = GeoLocation {
            country_code: Some("CN".to_string()),
            city: Some("Shanghai".to_string()),
            latitude: 31.2304,
            longitude: 121.4737,
        };
        
        let best = select_best_exit(&nodes, &client).unwrap();
        assert_eq!(best.name, "tokyo"); // Tokyo is closer to Shanghai
    }
}
