use std::convert::Infallible;

use base64::URL_SAFE;
use deadpool::managed::{Object, PoolError};

use crate::database::NotifyDatabaseManager;

#[derive(Clone)]
pub struct ServerState {
    pool: deadpool::managed::Pool<NotifyDatabaseManager>,
    pub server_base: String,
    pub vapid_pubkey: String,
    pub vapid_privkey: Vec<u8>,
}

impl ServerState {
    pub async fn new() -> Self {
        let pool = deadpool::managed::Pool::<NotifyDatabaseManager>::builder(NotifyDatabaseManager)
            .build()
            .unwrap();

        let vapid_pubkey =
            std::env::var("NOTIFY_VAPID_PUBKEY").expect("Expected NOTIFY_VAPID_PUBKEY env var.");
        let vapid_privkey_b64 =
            std::env::var("NOTIFY_VAPID_PRIVKEY").expect("Expected NOTIFY_VAPID_PRIVKEY env var.");
        let server_base =
            std::env::var("NOTIFY_API_SERVER").expect("Expected NOTIFY_API_SERVER env var.");

        let vapid_privkey = base64::decode_config(&vapid_privkey_b64, URL_SAFE)
            .expect("Could not decode VAPID private key as base64.");

        ServerState {
            pool,
            vapid_privkey,
            vapid_pubkey,
            server_base,
        }
    }

    pub async fn db(&self) -> Result<Object<NotifyDatabaseManager>, PoolError<Infallible>> {
        self.pool.get().await
    }
}
