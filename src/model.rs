use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Serialize, Deserialize, Debug)]
pub struct Subscription {
    pub endpoint: String,
    pub auth: String,
    pub p256dh: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Channel {
    #[serde(with="firestore_serde_timestamp::timestamp")]
    pub created: DateTime<Utc>,
    pub created_agent: String,
    pub created_ip: String,
}