use actix_web::{get, post, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_ws::Message;
use anyhow::{Context, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::unbounded_channel;
use tracing::{info, warn};

mod auth;
mod config;
mod metrics;
mod relay;
mod session;
mod tls;

use crate::auth::{parse_role, verify_pairing_key, Role};
use crate::config::Config;
use crate::metrics::Metrics;
use crate::relay::{RelayState, RouteResult};

struct AppState {
    relay: Arc<RelayState>,
    metrics: Arc<Metrics>,
    registered: dashmap::DashMap<String, String>, // pair_id -> pairing_key
}

#[derive(Deserialize)]
struct RegisterBody {
    pairing_key: String,
}

#[derive(Serialize)]
struct RegisterResponse {
    device_pair_id: String,
}

#[get("/health")]
async fn health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({ "status": "ok" }))
}

#[get("/metrics")]
async fn metrics_endpoint(state: web::Data<AppState>) -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(state.metrics.render())
}

#[post("/register")]
async fn register(state: web::Data<AppState>, body: web::Json<RegisterBody>) -> impl Responder {
    if body.pairing_key.len() < 16 {
        return HttpResponse::BadRequest().body("pairing_key too short");
    }
    let pair_id = format!("pair_{}", uuid::Uuid::new_v4().simple());
    state
        .registered
        .insert(pair_id.clone(), body.pairing_key.clone());
    HttpResponse::Ok().json(RegisterResponse {
        device_pair_id: pair_id,
    })
}

#[derive(Deserialize)]
struct WsQuery {
    role: String,
    pairing_key: String,
}

async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    path: web::Path<String>,
    query: web::Query<WsQuery>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, actix_web::Error> {
    let pair_id = path.into_inner();
    let role = match parse_role(&query.role) {
        Ok(r) => r,
        Err(_) => {
            state
                .metrics
                .auth_failures_total
                .fetch_add(1, Ordering::Relaxed);
            return Ok(HttpResponse::BadRequest().body("invalid role"));
        }
    };

    let expected = match state.registered.get(&pair_id) {
        Some(v) => v.clone(),
        None => {
            state
                .metrics
                .auth_failures_total
                .fetch_add(1, Ordering::Relaxed);
            return Ok(HttpResponse::NotFound().body("pair not registered"));
        }
    };
    if verify_pairing_key(&query.pairing_key, &expected).is_err() {
        state
            .metrics
            .auth_failures_total
            .fetch_add(1, Ordering::Relaxed);
        return Ok(HttpResponse::Unauthorized().body("invalid pairing key"));
    }

    state
        .metrics
        .connections_total
        .fetch_add(1, Ordering::Relaxed);

    let (response, mut ws_session, mut msg_stream) = actix_ws::handle(&req, stream)?;

    let (tx, mut rx) = unbounded_channel::<String>();
    let attached = state
        .relay
        .attach(&pair_id, expected.clone(), role, tx)
        .await;
    if !attached {
        state
            .metrics
            .auth_failures_total
            .fetch_add(1, Ordering::Relaxed);
        let _ = ws_session
            .close(Some(actix_ws::CloseReason {
                code: actix_ws::CloseCode::Other(4001),
                description: Some("auth failed".into()),
            }))
            .await;
        return Ok(response);
    }

    let relay = state.relay.clone();
    let metrics = state.metrics.clone();
    let pair_id_outbound = pair_id.clone();

    actix_web::rt::spawn(async move {
        // Outbound writer: forwards Tx channel to websocket.
        let mut outbound_session = ws_session.clone();
        let writer = actix_web::rt::spawn(async move {
            while let Some(body) = rx.recv().await {
                if outbound_session.text(body).await.is_err() {
                    break;
                }
            }
        });

        // Inbound reader
        while let Some(msg) = msg_stream.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    let s = text.to_string();
                    let result = match role {
                        Role::Android => relay.route_android_to_desktop(&pair_id_outbound, s).await,
                        Role::Desktop => relay.route_desktop_to_android(&pair_id_outbound, s).await,
                    };
                    match result {
                        RouteResult::Delivered => {
                            metrics
                                .messages_routed_total
                                .fetch_add(1, Ordering::Relaxed);
                        }
                        RouteResult::Queued => {
                            metrics
                                .messages_queued_total
                                .fetch_add(1, Ordering::Relaxed);
                        }
                        RouteResult::Dropped | RouteResult::NoSession | RouteResult::TooLarge => {
                            metrics
                                .messages_dropped_total
                                .fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                Ok(Message::Ping(b)) => {
                    let _ = ws_session.pong(&b).await;
                }
                Ok(Message::Close(_)) | Err(_) => break,
                _ => {}
            }
        }

        relay.detach(&pair_id_outbound, role).await;
        writer.abort();
        let _ = ws_session.close(None).await;
    });

    Ok(response)
}

#[actix_web::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config_path = std::env::args()
        .skip_while(|a| a != "--config")
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("config.toml"));

    let cfg = Config::load(&config_path).context("load config")?;
    info!(
        bind = %cfg.bind,
        ping_interval_secs = cfg.ping_interval_secs,
        rate_limit_per_min = cfg.rate_limit_per_min,
        "relay starting"
    );

    let state = web::Data::new(AppState {
        relay: Arc::new(RelayState::new(
            Duration::from_secs(cfg.pairing_ttl_secs),
            cfg.max_message_bytes,
            Duration::from_secs(cfg.ping_interval_secs),
            cfg.rate_limit_per_min,
        )),
        metrics: Arc::new(Metrics::default()),
        registered: dashmap::DashMap::new(),
    });

    let server_builder = HttpServer::new({
        let state = state.clone();
        move || {
            App::new()
                .app_data(state.clone())
                .service(health)
                .service(register)
                .service(metrics_endpoint)
                .route("/ws/{pair_id}", web::get().to(ws_handler))
        }
    });

    let cert = PathBuf::from(&cfg.tls_cert_path);
    let key = PathBuf::from(&cfg.tls_key_path);

    let server = if cert.exists() && key.exists() {
        let tls_cfg = tls::load_tls_config(&cert, &key)?;
        server_builder.bind_rustls_0_22(&cfg.bind, tls_cfg)?
    } else {
        warn!(
            "TLS cert/key missing at {} / {}; binding plaintext (dev only)",
            cert.display(),
            key.display()
        );
        server_builder.bind(&cfg.bind)?
    };

    server.run().await.context("server run")?;
    Ok(())
}
