use metrics::{counter, gauge, histogram};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

/// Install the Prometheus exporter and register all application metrics.
/// Returns a `PrometheusHandle` whose `render()` method produces the
/// text/plain Prometheus scrape payload.
pub fn init_metrics() -> PrometheusHandle {
    let builder = PrometheusBuilder::new();
    let handle = builder
        .install_recorder()
        .expect("failed to install Prometheus recorder");

    // Pre-register counters so they appear even before the first increment.
    counter!("trade_events_total").absolute(0);
    counter!("copy_signals_emitted").absolute(0);
    counter!("orders_filled").absolute(0);
    counter!("orders_failed").absolute(0);
    counter!("consensus_signals_total").absolute(0);

    // Pre-register gauges at zero.
    gauge!("active_whales").set(0.0);
    gauge!("open_positions").set(0.0);

    // Histogram is lazily created on first record; force creation.
    histogram!("pipeline_latency_seconds").record(0.0);

    handle
}
