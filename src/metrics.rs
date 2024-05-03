use std::net::SocketAddr;

use axum::{response::Html, routing::get, Router};
use log::info;
use once_cell::sync::Lazy;
use prometheus::{register_gauge_vec, Encoder, GaugeVec, TextEncoder};
use tokio::task::JoinHandle;

pub static METRIC_BALANCE_SOL: Lazy<GaugeVec> = Lazy::new(|| {
    register_gauge_vec!(
        "balance_sol",
        "Balance of SOL in a Solana account",
        &["name", "pubkey"]
    )
    .unwrap()
});

pub static METRIC_TOTAL_BALANCE_SOL: Lazy<GaugeVec> = Lazy::new(|| {
    register_gauge_vec!(
        "total_balance_sol",
        "Total balance of SOL in many Solana accounts",
        &["name"]
    )
    .unwrap()
});

pub fn update_metric_balance_sol(name: &str, pubkey: &str, lamports: f64) {
    METRIC_BALANCE_SOL
        .with_label_values(&[name, pubkey])
        .set(lamports);
}

pub fn update_metric_total_balance_sol(name: &str, lamports: f64) {
    METRIC_TOTAL_BALANCE_SOL
        .with_label_values(&[name])
        .set(lamports);
}

pub fn reset_metric_balance_sol() {
    METRIC_BALANCE_SOL.reset();
}

pub fn remove_metric_total_balance_sol(name: &str) {
    let _ = METRIC_TOTAL_BALANCE_SOL.remove_label_values(&[name]);
}

async fn handler() -> Html<String> {
    let mut buffer = Vec::new();
    TextEncoder::new()
        .encode(&prometheus::gather(), &mut buffer)
        .unwrap();

    Html(String::from_utf8(buffer.clone()).unwrap())
}

pub fn spawn_metrics_server(port: u16) -> JoinHandle<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Serving metrics on {}", addr);

    tokio::spawn(async move {
        axum::Server::bind(&addr)
            .serve(
                Router::new()
                    .route("/metrics", get(handler))
                    .into_make_service(),
            )
            .await
            .unwrap();

        info!("Metrics server exitted cleanly");
    })
}
