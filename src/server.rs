use crate::logging::LogError;
use crate::model::{
    Channel, Message, MessageResult, Subscription, MESSAGES_COLLECTION, SUBSCRIPTIONS_COLLECTION,
};
use crate::rate_limiter::RateLimiterMiddleware;
use crate::server_state::ServerState;
use crate::vapid::{send_message, MessagePayload};
use axum::body::{Body, Bytes};
use axum::extract::{ConnectInfo, TypedHeader};
use axum::http::{Response, Uri};
use axum::routing::BoxRoute;
use axum::service;
use axum::{
    extract::{Extension, Path},
    handler::{get, post},
    http::StatusCode,
    AddExtensionLayer, Json, Router,
};
use chrono::{DateTime, Utc};
use futures::future::join_all;
use governor::Quota;
use headers::{HeaderMap, HeaderName, HeaderValue, UserAgent};
use nonzero_ext::nonzero;
use qrcode::render::svg;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;
use tiny_firestore_odm::Collection;
use tokio::time::timeout;
use tower::layer::layer_fn;
use tower_http::services::ServeDir;
use tower_http::services::ServeFile;

/// Timeout (seconds) of external service when invoking push request.
const TIMEOUT_SECS: u64 = 10;

/// Rate limit on calls that access database.
const MAX_REQUESTS_PER_MINUTE: u32 = 20;

#[derive(Serialize)]
struct MessageInfo {
    message: String,
    result: Vec<MessageResult>,
    time: DateTime<Utc>,
}

#[derive(Serialize)]
struct ChannelInfo {
    #[serde(rename = "channelId")]
    channel_id: String,

    messages: Vec<MessageInfo>,

    time: String,

    #[serde(rename = "pubKey")]
    pub_key: String,

    endpoint: String,
    channel_page: String,
}

async fn register_channel(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    TypedHeader(user_agent): TypedHeader<UserAgent>,
    server_state: Extension<ServerState>,
) -> Result<Json<ChannelInfo>, StatusCode> {
    let db = server_state.db().await.log_error_internal()?;
    let ip: String = addr.ip().to_string();

    let channels = db.channels();

    let channel_id = channels
        .create(&Channel {
            created: Utc::now(),
            created_agent: user_agent.to_string(),
            created_ip: ip.clone(),
        })
        .await
        .log_error_internal()?
        .leaf_name()
        .to_string();

    tracing::info!(%channel_id, %ip, "Channel created.");

    Ok(Json(ChannelInfo {
        messages: Vec::new(),
        time: "".to_string(),
        pub_key: server_state.vapid_pubkey.to_string(),
        endpoint: server_state.endpoint_url(&channel_id),
        channel_page: server_state.channel_page_url(&channel_id),
        channel_id,
    }))
}

async fn info(
    server_state: Extension<ServerState>,
    Path(channel_id): Path<String>,
) -> Result<Json<ChannelInfo>, StatusCode> {
    let db = server_state.db().await.log_error_internal()?;

    let channels = db.channels();
    channels.get(&*channel_id).await.log_error_not_found()?;

    let messages: Collection<Message> = channels.subcollection(&channel_id, MESSAGES_COLLECTION);

    let messages = messages
        .list()
        .with_order_by("message_time desc")
        .with_page_size(10)
        .get_page()
        .await;

    Ok(Json(ChannelInfo {
        messages: messages
            .into_iter()
            .map(|d| MessageInfo {
                message: d.value.message,
                result: d.value.result,
                time: d.value.message_time,
            })
            .collect(),
        time: "".to_string(),
        pub_key: server_state.vapid_pubkey.to_string(),
        endpoint: server_state.endpoint_url(&channel_id),
        channel_page: server_state.channel_page_url(&channel_id),
        channel_id,
    }))
}

async fn send_message_with_timeout(
    payload: &MessagePayload,
    subscription: Subscription,
    privkey: &[u8],
    duration: Duration,
) -> MessageResult {
    let result = timeout(duration, send_message(payload, &subscription, privkey)).await;

    let result_status = match result {
        Ok(Ok(_)) => "201".to_string(),
        Ok(Err(e)) => e.to_string(),
        Err(e) => "Timed out.".to_string(),
    };

    let endpoint_domain = Uri::from_str(&subscription.endpoint)
        .ok()
        .map(|d| d.authority().map(|d| d.to_string()))
        .flatten()
        .unwrap_or_default();

    MessageResult {
        result_status,
        endpoint_domain,
    }
}

async fn send(
    server_state: Extension<ServerState>,
    Path(channel_id): Path<String>,
    message: String,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<String, StatusCode> {
    let db = server_state.db().await.log_error_internal()?;

    let channels = db.channels();
    channels.get(&*channel_id).await.log_error_not_found()?;

    // Send to subscriptions.

    let subscriptions: Collection<Subscription> =
        channels.subcollection(&channel_id, SUBSCRIPTIONS_COLLECTION);

    let payload = MessagePayload::parse_new(
        &message,
        &*channel_id,
        &server_state.channel_page_url(&*channel_id),
    );
    // let mut message_result = Vec::new();
    let mut futures = Vec::new();

    let subscriptions = subscriptions.list().with_page_size(10).get_page().await;
    for subscription in subscriptions {
        futures.push(send_message_with_timeout(
            &payload,
            subscription.value,
            &server_state.vapid_privkey,
            Duration::from_secs(TIMEOUT_SECS),
        ));
    }

    let message_result = join_all(futures.into_iter()).await;

    tracing::info!(%channel_id, ?message_result, "Message sent.");

    // Store message.
    let messages: Collection<Message> = channels.subcollection(&channel_id, MESSAGES_COLLECTION);

    messages
        .create(&Message {
            message: payload.message.to_string(),
            message_time: Utc::now(),
            sender_ip: addr.ip().to_string(),
            result: message_result,
        })
        .await
        .log_error_internal()?;

    Ok("ok".to_string())
}

#[derive(Deserialize)]
struct SubscriptionRequestKeys {
    auth: String,
    p256dh: String,
}

#[derive(Deserialize)]
struct SubscriptionRequestSubscription {
    endpoint: String,
    keys: SubscriptionRequestKeys,
}

#[derive(Deserialize)]
struct SubscriptionRequest {
    id: String,
    subscription: SubscriptionRequestSubscription,
}

async fn subscribe(
    subscription: Json<SubscriptionRequest>,
    server_state: Extension<ServerState>,
    Path(channel_id): Path<String>,
) -> Result<Json<()>, StatusCode> {
    let db = server_state.db().await.log_error_internal()?;

    let channels = db.channels();
    channels.get(&*channel_id).await.log_error_not_found()?;

    let subscriptions: Collection<Subscription> =
        channels.subcollection(&channel_id, SUBSCRIPTIONS_COLLECTION);

    let subscription_id = subscription.id.clone();

    subscriptions
        .try_create(
            &Subscription {
                endpoint: subscription.0.subscription.endpoint,
                auth: subscription.0.subscription.keys.auth,
                p256dh: subscription.0.subscription.keys.p256dh,
            },
            &*subscription_id,
        )
        .await
        .log_error_internal()?;

    Ok(Json(()))
}

async fn render_qr_code(
    server_state: Extension<ServerState>,
    Path(channel_id): Path<String>,
) -> (HeaderMap, Bytes) {
    let url = format!("{}/c/{}", server_state.server_base, channel_id);

    let img: String = qrcode::QrCode::new(url.as_bytes())
        .unwrap()
        .render()
        .min_dimensions(200, 200)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build();

    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("content-type"),
        HeaderValue::from_static("image/svg+xml"),
    );

    let b = Bytes::from(img);

    (headers, b)
}

fn static_routes() -> Router<BoxRoute> {
    Router::new()
        .nest(
            "/",
            service::get(ServeFile::new("static/index.html"))
                .handle_error(|_| Ok::<_, Infallible>(StatusCode::NOT_FOUND)),
        )
        .nest(
            "/static",
            service::get(ServeDir::new("static/"))
                .handle_error(|_| Ok::<_, Infallible>(StatusCode::NOT_FOUND)),
        )
        .nest(
            "/c/:channel_id",
            service::get(ServeFile::new("static/channel.html"))
                .handle_error(|_| Ok::<_, Infallible>(StatusCode::NOT_FOUND)),
        )
        .boxed()
}

pub async fn redirect(
    server_state: Extension<ServerState>,
    Path(channel_id): Path<String>,
) -> Response<Body> {
    if channel_id.len() > 6 && channel_id.chars().all(|c| char::is_ascii_alphanumeric(&c)) {
        let new_location = server_state.channel_page_url(&*channel_id);
        Response::builder()
            .status(302)
            .header(
                HeaderName::from_static("location"),
                HeaderValue::from_str(&new_location).unwrap(),
            )
            .body(Body::empty())
            .unwrap()
    } else {
        Response::builder().status(404).body(Body::empty()).unwrap()
    }
}

/// Bad JavaScript clients access /undefined so frequently that we short-circuit it.
pub async fn undefined() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "No such channel.")
}

/// The old service worker was not in the /static/ directory and still persists on some clients.
pub async fn moved_service_worker(server_state: Extension<ServerState>) -> Response<Body> {
    Response::builder()
        .status(301)
        .header(
            HeaderName::from_static("location"),
            HeaderValue::from_str(&format!(
                "{}/static/service-worker.js",
                server_state.server_base
            ))
            .unwrap(),
        )
        .body(Body::empty())
        .unwrap()
}

fn active_routes() -> Router<BoxRoute> {
    Router::new()
        .route("/:channel_id/json", get(info))
        .route("/:channel_id/subscribe", post(subscribe))
        .route("/api/register_channel", post(register_channel))
        .route("/register_channel", post(register_channel)) // Used by py client.
        .layer(layer_fn(|inner| {
            RateLimiterMiddleware::new(inner, Quota::per_minute(nonzero!(MAX_REQUESTS_PER_MINUTE)))
        }))
        .boxed()
}

pub async fn serve(port: Option<u16>) -> anyhow::Result<()> {
    let port: u16 = if let Some(port) = port {
        port
    } else if let Ok(port) = std::env::var("PORT") {
        port.parse()?
    } else {
        8080
    };

    let server_state = ServerState::new().await;

    let app = Router::new()
        .nest("/", static_routes())
        .route("/:channel_id", get(redirect).post(send))
        .route("/undefined", get(undefined).post(undefined))
        .route("/service-worker.js", get(moved_service_worker))
        .route("/:channel_id/qr.svg", get(render_qr_code))
        .nest("/", active_routes())
        .layer(AddExtensionLayer::new(server_state));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr, _>())
        .await?;

    Ok(())
}
