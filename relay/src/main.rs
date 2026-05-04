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

mod account_store;
mod auth;
mod config;
mod metrics;
mod relay;
mod session;
mod tls;

use crate::account_store::{AccountStore, LoginError, RegisterError};
use crate::auth::{
    create_session_token, normalize_email, parse_role, verify_pairing_key, verify_session_token,
    AuthError, Role,
};
use crate::config::Config;
use crate::metrics::Metrics;
use crate::relay::{RelayState, RouteResult};

struct AppState {
    relay: Arc<RelayState>,
    metrics: Arc<Metrics>,
    registered: dashmap::DashMap<String, String>, // pair_id -> pairing_key
    accounts: Arc<AccountStore>,
    auth_token_secret: String,
    google_client_id: Option<String>,
    resend_api_key: Option<String>,
    otp_email_from: Option<String>,
    pending_otps: dashmap::DashMap<String, PendingOtp>,
}

#[derive(Deserialize)]
struct RegisterBody {
    pairing_key: String,
}

#[derive(Deserialize)]
struct AccountRegisterBody {
    email: String,
    password: String,
}

#[derive(Deserialize)]
struct AccountLoginBody {
    email: String,
    password: String,
}

#[derive(Clone)]
struct PendingOtp {
    password: String,
    code: String,
    expires_at_epoch_secs: u64,
}

#[derive(Deserialize)]
struct OtpStartBody {
    email: String,
    password: String,
}

#[derive(Deserialize)]
struct OtpVerifyBody {
    email: String,
    password: String,
    otp: String,
}

#[derive(Deserialize)]
struct GoogleLoginBody {
    id_token: String,
}

#[derive(Serialize)]
struct RegisterResponse {
    device_pair_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthResponse {
    user_id: String,
    email: String,
    token: String,
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
async fn register(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<RegisterBody>,
) -> impl Responder {
    if authenticate_bearer(&req, &state).is_err() {
        return HttpResponse::Unauthorized().body("missing or invalid bearer token");
    }
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

#[post("/auth/register")]
async fn auth_register(
    state: web::Data<AppState>,
    body: web::Json<AccountRegisterBody>,
) -> impl Responder {
    match state
        .accounts
        .register_password(&body.email, &body.password, now_epoch_secs())
    {
        Ok(user) => HttpResponse::Ok().json(auth_response(&state, &user.id, &user.email)),
        Err(RegisterError::AlreadyExists) => {
            HttpResponse::Conflict().body("account already exists")
        }
        Err(RegisterError::Auth(AuthError::InvalidEmail)) => {
            HttpResponse::BadRequest().body("invalid email")
        }
        Err(RegisterError::Auth(AuthError::WeakPassword)) => {
            HttpResponse::BadRequest().body("password must be at least 10 characters")
        }
        Err(err) => {
            warn!(error = %err, "account registration failed");
            HttpResponse::InternalServerError().body("account registration failed")
        }
    }
}

#[post("/auth/login")]
async fn auth_login(
    state: web::Data<AppState>,
    body: web::Json<AccountLoginBody>,
) -> impl Responder {
    match state.accounts.login_password(&body.email, &body.password) {
        Ok(user) => HttpResponse::Ok().json(auth_response(&state, &user.id, &user.email)),
        Err(LoginError::InvalidCredentials) => {
            HttpResponse::Unauthorized().body("invalid email or password")
        }
    }
}

#[post("/auth/otp/start")]
async fn auth_otp_start(
    state: web::Data<AppState>,
    body: web::Json<OtpStartBody>,
) -> impl Responder {
    let email = match normalize_email(&body.email) {
        Ok(email) => email,
        Err(_) => return HttpResponse::BadRequest().body("invalid email"),
    };
    if body.password.len() < 10 {
        return HttpResponse::BadRequest().body("password must be at least 10 characters");
    }
    let code = format!("{:06}", rand::random::<u32>() % 1_000_000);
    let expires_at = now_epoch_secs() + 10 * 60;
    if let Err(err) = send_otp_email(&state, &email, &code).await {
        warn!(error = %err, "otp email send failed");
        return HttpResponse::ServiceUnavailable().body(err);
    }
    state.pending_otps.insert(
        email.clone(),
        PendingOtp {
            password: body.password.clone(),
            code,
            expires_at_epoch_secs: expires_at,
        },
    );
    HttpResponse::Ok().json(serde_json::json!({ "email": email, "expiresInSeconds": 600 }))
}

#[post("/auth/otp/verify")]
async fn auth_otp_verify(
    state: web::Data<AppState>,
    body: web::Json<OtpVerifyBody>,
) -> impl Responder {
    let email = match normalize_email(&body.email) {
        Ok(email) => email,
        Err(_) => return HttpResponse::BadRequest().body("invalid email"),
    };
    let Some((_, pending)) = state.pending_otps.remove(&email) else {
        return HttpResponse::Unauthorized().body("otp is missing or expired");
    };
    if pending.expires_at_epoch_secs < now_epoch_secs() {
        return HttpResponse::Unauthorized().body("otp is expired");
    }
    if pending.password != body.password || pending.code != body.otp.trim() {
        return HttpResponse::Unauthorized().body("invalid otp");
    }
    match state
        .accounts
        .register_password(&email, &body.password, now_epoch_secs())
    {
        Ok(user) => HttpResponse::Ok().json(auth_response(&state, &user.id, &user.email)),
        Err(RegisterError::AlreadyExists) => {
            match state.accounts.login_password(&email, &body.password) {
                Ok(user) => HttpResponse::Ok().json(auth_response(&state, &user.id, &user.email)),
                Err(_) => HttpResponse::Conflict().body("account already exists"),
            }
        }
        Err(RegisterError::Auth(AuthError::WeakPassword)) => {
            HttpResponse::BadRequest().body("password must be at least 10 characters")
        }
        Err(err) => {
            warn!(error = %err, "otp account registration failed");
            HttpResponse::InternalServerError().body("otp registration failed")
        }
    }
}

#[derive(Debug, Deserialize)]
struct GoogleTokenInfo {
    aud: String,
    email: Option<String>,
    email_verified: Option<String>,
}

#[post("/auth/google")]
async fn auth_google(
    state: web::Data<AppState>,
    body: web::Json<GoogleLoginBody>,
) -> impl Responder {
    let Some(client_id) = &state.google_client_id else {
        return HttpResponse::ServiceUnavailable().body("google auth is not configured");
    };
    let response = reqwest::Client::new()
        .get("https://oauth2.googleapis.com/tokeninfo")
        .query(&[("id_token", body.id_token.as_str())])
        .send()
        .await;
    let Ok(response) = response else {
        return HttpResponse::BadGateway().body("google token verification unavailable");
    };
    if !response.status().is_success() {
        return HttpResponse::Unauthorized().body("invalid google token");
    }
    let Ok(token_info) = response.json::<GoogleTokenInfo>().await else {
        return HttpResponse::Unauthorized().body("invalid google token response");
    };
    if token_info.aud != *client_id || token_info.email_verified.as_deref() != Some("true") {
        return HttpResponse::Unauthorized().body("google token audience or email is invalid");
    }
    let Some(email) = token_info.email else {
        return HttpResponse::Unauthorized().body("google token has no email");
    };
    match state.accounts.upsert_google_user(&email, now_epoch_secs()) {
        Ok(user) => HttpResponse::Ok().json(auth_response(&state, &user.id, &user.email)),
        Err(err) => {
            warn!(error = %err, "google account upsert failed");
            HttpResponse::InternalServerError().body("google login failed")
        }
    }
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
    load_local_env_files();

    let config_path = std::env::args()
        .skip_while(|a| a != "--config")
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("config.toml"));

    let cfg = Config::load(&config_path).context("load config")?;
    let auth_token_secret = cfg.auth_token_secret.clone().unwrap_or_else(|| {
        warn!("FOCUSBRIDGE_AUTH_TOKEN_SECRET is not configured; generated sessions will reset on restart");
        format!("dev_{}", uuid::Uuid::new_v4().simple())
    });
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
        accounts: Arc::new(AccountStore::load(&cfg.auth_store_path)?),
        auth_token_secret,
        google_client_id: cfg.google_client_id.clone(),
        resend_api_key: cfg.resend_api_key.clone(),
        otp_email_from: cfg.otp_email_from.clone(),
        pending_otps: dashmap::DashMap::new(),
    });

    let server_builder = HttpServer::new({
        let state = state.clone();
        move || {
            App::new()
                .app_data(state.clone())
                .service(health)
                .service(auth_register)
                .service(auth_login)
                .service(auth_otp_start)
                .service(auth_otp_verify)
                .service(auth_google)
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

fn authenticate_bearer(req: &HttpRequest, state: &AppState) -> Result<String, AuthError> {
    let header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(AuthError::InvalidToken)?;
    let token = header
        .strip_prefix("Bearer ")
        .ok_or(AuthError::InvalidToken)?;
    verify_session_token(token, &state.auth_token_secret, now_epoch_secs())
}

fn auth_response(state: &AppState, user_id: &str, email: &str) -> AuthResponse {
    AuthResponse {
        user_id: user_id.into(),
        email: email.into(),
        token: create_session_token(
            user_id,
            &state.auth_token_secret,
            now_epoch_secs(),
            30 * 24 * 60 * 60,
        ),
    }
}

async fn send_otp_email(
    state: &AppState,
    email: &str,
    code: &str,
) -> std::result::Result<(), String> {
    let Some(api_key) = &state.resend_api_key else {
        return Err("OTP email is not configured: set FOCUSBRIDGE_RESEND_API_KEY".into());
    };
    let Some(from) = &state.otp_email_from else {
        return Err("OTP email is not configured: set FOCUSBRIDGE_OTP_EMAIL_FROM".into());
    };
    let response = reqwest::Client::new()
        .post("https://api.resend.com/emails")
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "from": from,
            "to": [email],
            "subject": "Your FocusBridge verification code",
            "text": format!("Your FocusBridge verification code is {code}. It expires in 10 minutes.")
        }))
        .send()
        .await
        .map_err(|e| format!("send otp email request failed: {e}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("otp email provider failed ({status}): {body}"));
    }
    Ok(())
}

fn now_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn load_local_env_files() {
    for path in [PathBuf::from(".env.local"), PathBuf::from("../.env.local")] {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            if key.is_empty() || std::env::var_os(key).is_some() {
                continue;
            }
            let value = value.trim().trim_matches('"');
            std::env::set_var(key, value);
        }
    }
}
