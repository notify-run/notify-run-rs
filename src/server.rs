use crate::logging::LogError;
use crate::model::{
    Channel, Message, MessageResult, Subscription, MESSAGES_COLLECTION, SUBSCRIPTIONS_COLLECTION,
};
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
use headers::{HeaderMap, HeaderName, HeaderValue, UserAgent};
use qrcode::render::svg;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::str::FromStr;
use tiny_firestore_odm::Collection;
use tower_http::services::ServeDir;
use tower_http::services::ServeFile;

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
            created_ip: ip,
        })
        .await
        .log_error_internal()?;

    Ok(Json(ChannelInfo {
        channel_id: channel_id.leaf_name().to_string(),
        messages: Vec::new(),
        time: "".to_string(),
        pub_key: server_state.vapid_pubkey.to_string(),
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
        channel_id,
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
    }))
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

    let payload = MessagePayload::new(&message, &*channel_id);
    let mut message_result = Vec::new();

    for subscription in subscriptions.list().with_page_size(10).get_page().await {
        let result = send_message(&payload, &subscription.value, &server_state.vapid_privkey).await;

        let result_status = match result {
            Ok(_) => "201".to_string(),
            Err(e) => e.to_string(),
        };

        let endpoint_domain = Uri::from_str(&subscription.value.endpoint)
            .ok()
            .map(|d| d.authority().map(|d| d.to_string()))
            .flatten()
            .unwrap_or_default();

        message_result.push(MessageResult {
            result_status,
            endpoint_domain,
        });
    }

    // Store message.
    let messages: Collection<Message> = channels.subcollection(&channel_id, MESSAGES_COLLECTION);

    messages
        .create(&Message {
            message,
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
) -> Result<String, StatusCode> {
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

    Ok("".to_string())
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
        let new_location = format!("{}/c/{}", server_state.server_base, channel_id);
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
        .route("/:channel_id/json", get(info))
        .route("/:channel_id/subscribe", post(subscribe))
        .route("/:channel_id/qr.svg", get(render_qr_code))
        .route("/api/register_channel", post(register_channel))
        .route("/:channel_id", get(redirect).post(send))
        .layer(AddExtensionLayer::new(server_state));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr, _>())
        .await?;

    Ok(())
}
