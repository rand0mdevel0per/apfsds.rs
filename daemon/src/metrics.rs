//! Prometheus metrics

use crate::config::MonitoringConfig;
use prometheus::{Histogram, HistogramOpts, IntCounter, IntGauge, Opts, Registry};
use std::sync::LazyLock;
use tokio::task::JoinHandle;
use tracing::{error, info};

/// Global metrics registry
static REGISTRY: LazyLock<Registry> = LazyLock::new(Registry::new);

/// Metrics struct
pub struct Metrics {
    // Counters
    pub frames_sent: IntCounter,
    pub frames_received: IntCounter,
    pub auth_successes: IntCounter,
    pub auth_failures: IntCounter,

    // Gauges
    pub active_connections: IntGauge,
    pub pool_connections: IntGauge,

    // Histograms
    pub request_duration: Histogram,
    pub frame_size: Histogram,
}

impl Metrics {
    pub fn new() -> Self {
        let frames_sent = IntCounter::with_opts(Opts::new(
            "apfsds_frames_sent_total",
            "Total number of frames sent",
        ))
        .unwrap();

        let frames_received = IntCounter::with_opts(Opts::new(
            "apfsds_frames_received_total",
            "Total number of frames received",
        ))
        .unwrap();

        let auth_successes = IntCounter::with_opts(Opts::new(
            "apfsds_auth_successes_total",
            "Total successful authentications",
        ))
        .unwrap();

        let auth_failures = IntCounter::with_opts(Opts::new(
            "apfsds_auth_failures_total",
            "Total failed authentications",
        ))
        .unwrap();

        let active_connections = IntGauge::with_opts(Opts::new(
            "apfsds_active_connections",
            "Number of active connections",
        ))
        .unwrap();

        let pool_connections = IntGauge::with_opts(Opts::new(
            "apfsds_pool_connections",
            "Number of pooled connections",
        ))
        .unwrap();

        let request_duration = Histogram::with_opts(HistogramOpts::new(
            "apfsds_request_duration_seconds",
            "Request duration in seconds",
        ))
        .unwrap();

        let frame_size = Histogram::with_opts(
            HistogramOpts::new("apfsds_frame_size_bytes", "Frame size in bytes")
                .buckets(vec![64.0, 256.0, 512.0, 1024.0, 4096.0, 8192.0, 16384.0]),
        )
        .unwrap();

        // Register metrics
        REGISTRY.register(Box::new(frames_sent.clone())).ok();
        REGISTRY.register(Box::new(frames_received.clone())).ok();
        REGISTRY.register(Box::new(auth_successes.clone())).ok();
        REGISTRY.register(Box::new(auth_failures.clone())).ok();
        REGISTRY.register(Box::new(active_connections.clone())).ok();
        REGISTRY.register(Box::new(pool_connections.clone())).ok();
        REGISTRY.register(Box::new(request_duration.clone())).ok();
        REGISTRY.register(Box::new(frame_size.clone())).ok();

        Self {
            frames_sent,
            frames_received,
            auth_successes,
            auth_failures,
            active_connections,
            pool_connections,
            request_duration,
            frame_size,
        }
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Start the metrics server
pub fn start_server(config: &MonitoringConfig) -> JoinHandle<()> {
    let bind = config.prometheus_bind;
    let enabled = config.prometheus_enabled;

    tokio::spawn(async move {
        if !enabled {
            info!("Prometheus metrics disabled");
            return;
        }

        use bytes::Bytes;
        use http_body_util::Full;
        use hyper::{Response, server::conn::http1, service::service_fn};
        use hyper_util::rt::TokioIo;

        let listener = match tokio::net::TcpListener::bind(bind).await {
            Ok(l) => l,
            Err(e) => {
                error!("Failed to bind metrics server: {}", e);
                return;
            }
        };

        info!("Prometheus metrics server listening on {}", bind);

        loop {
            let (stream, _) = match listener.accept().await {
                Ok(r) => r,
                Err(e) => {
                    error!("Metrics accept error: {}", e);
                    continue;
                }
            };

            let io = TokioIo::new(stream);

            tokio::spawn(async move {
                let service = service_fn(|_req| async {
                    use prometheus::Encoder;

                    let encoder = prometheus::TextEncoder::new();
                    let mut buffer = Vec::new();
                    encoder.encode(&REGISTRY.gather(), &mut buffer).unwrap();

                    Ok::<_, std::convert::Infallible>(
                        Response::builder()
                            .header("Content-Type", "text/plain")
                            .body(Full::new(Bytes::from(buffer)))
                            .unwrap(),
                    )
                });

                if let Err(e) = http1::Builder::new().serve_connection(io, service).await {
                    error!("Metrics connection error: {}", e);
                }
            });
        }
    })
}
