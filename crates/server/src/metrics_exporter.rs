use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::sync::OnceLock;

static HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

pub fn init_metrics() {
    if HANDLE.get().is_some() {
        return;
    }
    let builder = PrometheusBuilder::new();
    let handle = builder
        .install_recorder()
        .expect("install prometheus recorder");
    let _ = HANDLE.set(handle);
}

pub fn render_metrics() -> String {
    HANDLE.get().map(|h| h.render()).unwrap_or_default()
}
