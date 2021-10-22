use std::{convert::Infallible, net::SocketAddr, sync::Arc};
use deadpool::managed;
use google_authz::TokenSource;
use serde::Serialize;
use axum::{Json, Router, extract::Path, handler::get, http::{StatusCode}};
use tiny_firestore_odm::{Collection, Database};
use async_trait::async_trait;
use crate::{get_creds_and_project, model::Channel};

async fn status() -> &'static str {
    "ok"
}

#[derive(Serialize)]
struct VapidResult {
    endpoint_domain: String,
    result_message: String,
    result_status: String,
    subscription: String,
}

#[derive(Serialize)]
struct MessageInfo {
    message: String,
    result: Vec<VapidResult>,
}

#[derive(Serialize)]
struct ChannelInfo {
    #[serde(rename="channelId")]
    channel_id: String,

    messages: Vec<MessageInfo>,

    time: String,
}

async fn info(Path(channel_id): Path<String>) -> Result<Json<ChannelInfo>, StatusCode> {
    Ok(Json(ChannelInfo {
        channel_id,
        messages: Vec::new(),
        time: "".to_string(),
    }))
}

struct NotifyDatabase {
    db: Database,
}

impl NotifyDatabase {
    pub fn channels(&self) -> Collection<Channel> {
        self.db.collection("channels")
    }
}

struct NotifyDatabaseManager {
    token_source: Arc<TokenSource>,
    project_id: String,
}

impl NotifyDatabaseManager {
    pub fn new(token_source: Arc<TokenSource>, project_id: &str) -> Self {
        NotifyDatabaseManager { token_source, project_id: project_id.to_string() }
    }
}

#[async_trait]
impl managed::Manager for NotifyDatabaseManager {
    type Type = NotifyDatabase;
    type Error = Infallible;
    
    async fn create(&self) -> Result<NotifyDatabase, Infallible> {
        let db = Database::new(self.token_source, &self.project_id).await;

        Ok(NotifyDatabase { db })
    }
    
    async fn recycle(&self, _: &mut NotifyDatabase) -> managed::RecycleResult<Infallible> {
        Ok(())
    }
}

struct ServerState {
    pool: deadpool::managed::Pool<NotifyDatabaseManager>,
}

impl ServerState {
    pub async fn new() -> Self {
        let (token_source, project_id) = get_creds_and_project().await;
        let manager = NotifyDatabaseManager::new(token_source, &project_id);
        let pool = deadpool::managed::Pool::<NotifyDatabaseManager>::builder(manager).build().unwrap();
        
        ServerState {
            pool
        }
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

    let app = Router::new()
        .route("/", get(status))
        .route("/:channel_id/json", get(info))
        ;

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
