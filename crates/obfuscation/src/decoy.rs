//! Decoy traffic generation to mimic real browser behavior

use std::time::Duration;

/// Resource type for decoy requests
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    Html,
    Css,
    JavaScript,
    Image,
    Font,
    Json,
    Xml,
}

impl ResourceType {
    /// Get typical file extension for this resource type
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Css => "css",
            Self::JavaScript => "js",
            Self::Image => "png",
            Self::Font => "woff2",
            Self::Json => "json",
            Self::Xml => "xml",
        }
    }

    /// Get typical Content-Type for this resource type
    pub fn content_type(&self) -> &'static str {
        match self {
            Self::Html => "text/html",
            Self::Css => "text/css",
            Self::JavaScript => "application/javascript",
            Self::Image => "image/png",
            Self::Font => "font/woff2",
            Self::Json => "application/json",
            Self::Xml => "application/xml",
        }
    }

    /// Get typical size range for this resource type (in bytes)
    pub fn size_range(&self) -> (usize, usize) {
        match self {
            Self::Html => (1024, 51200),        // 1KB - 50KB
            Self::Css => (512, 102400),         // 512B - 100KB
            Self::JavaScript => (1024, 512000), // 1KB - 500KB
            Self::Image => (2048, 1048576),     // 2KB - 1MB
            Self::Font => (10240, 204800),      // 10KB - 200KB
            Self::Json => (128, 10240),         // 128B - 10KB
            Self::Xml => (256, 20480),          // 256B - 20KB
        }
    }
}

/// Decoy traffic configuration
#[derive(Debug, Clone)]
pub struct DecoyConfig {
    /// Enable decoy traffic
    pub enabled: bool,

    /// Decoy request endpoints (paths)
    pub endpoints: Vec<String>,

    /// Request interval range (seconds)
    pub interval: (u64, u64),

    /// Size range for decoy requests (bytes)
    pub size_range: (usize, usize),

    /// Resource types to simulate
    pub resource_types: Vec<ResourceType>,
}

impl Default for DecoyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoints: vec![
                "/static/main.css".to_string(),
                "/static/app.js".to_string(),
                "/api/status".to_string(),
                "/favicon.ico".to_string(),
            ],
            interval: (30, 120), // 30-120 seconds
            size_range: (512, 10240),
            resource_types: vec![
                ResourceType::Css,
                ResourceType::JavaScript,
                ResourceType::Json,
                ResourceType::Image,
            ],
        }
    }
}

impl DecoyConfig {
    /// Generate a random interval for next decoy request
    pub fn random_interval(&self) -> Duration {
        let (min, max) = self.interval;
        Duration::from_secs(fastrand::u64(min..=max))
    }

    /// Generate a random size for decoy request
    pub fn random_size(&self) -> usize {
        let (min, max) = self.size_range;
        fastrand::usize(min..=max)
    }

    /// Select a random endpoint
    pub fn random_endpoint(&self) -> Option<&str> {
        if self.endpoints.is_empty() {
            None
        } else {
            let idx = fastrand::usize(0..self.endpoints.len());
            Some(&self.endpoints[idx])
        }
    }

    /// Select a random resource type
    pub fn random_resource_type(&self) -> Option<ResourceType> {
        if self.resource_types.is_empty() {
            None
        } else {
            let idx = fastrand::usize(0..self.resource_types.len());
            Some(self.resource_types[idx])
        }
    }

    /// Generate a realistic decoy request path
    pub fn generate_decoy_path(&self) -> String {
        if let Some(endpoint) = self.random_endpoint() {
            // Add random query parameters to make it look more realistic
            if fastrand::bool() {
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                format!("{}?_={}", endpoint, timestamp)
            } else {
                endpoint.to_string()
            }
        } else {
            // Fallback: generate a random path
            let resource_type = self.random_resource_type().unwrap_or(ResourceType::Json);
            format!(
                "/static/resource_{}.{}",
                fastrand::u32(..),
                resource_type.extension()
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_type_properties() {
        let rt = ResourceType::JavaScript;
        assert_eq!(rt.extension(), "js");
        assert_eq!(rt.content_type(), "application/javascript");
        let (min, max) = rt.size_range();
        assert!(min < max);
    }

    #[test]
    fn test_decoy_config_default() {
        let config = DecoyConfig::default();
        assert!(!config.enabled);
        assert!(!config.endpoints.is_empty());
        assert!(!config.resource_types.is_empty());
    }

    #[test]
    fn test_random_generation() {
        let config = DecoyConfig::default();

        // Test interval generation
        let interval = config.random_interval();
        assert!(interval.as_secs() >= config.interval.0);
        assert!(interval.as_secs() <= config.interval.1);

        // Test size generation
        let size = config.random_size();
        assert!(size >= config.size_range.0);
        assert!(size <= config.size_range.1);

        // Test endpoint selection
        assert!(config.random_endpoint().is_some());

        // Test resource type selection
        assert!(config.random_resource_type().is_some());

        // Test path generation
        let path = config.generate_decoy_path();
        assert!(!path.is_empty());
    }
}
