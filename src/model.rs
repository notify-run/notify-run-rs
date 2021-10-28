use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const MESSAGES_COLLECTION: &str = "messages";
pub const SUBSCRIPTIONS_COLLECTION: &str = "subscriptions";

#[derive(Serialize, Deserialize, Debug)]
pub struct Subscription {
    pub endpoint: String,
    pub auth: String,
    pub p256dh: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Channel {
    #[serde(with = "firestore_serde_timestamp::timestamp")]
    pub created: DateTime<Utc>,

    pub created_agent: String,
    pub created_ip: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Message {
    pub message: String,
    pub sender_ip: String,

    #[serde(with = "firestore_serde_timestamp::timestamp")]
    pub message_time: DateTime<Utc>,

    pub result: Vec<MessageResult>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MessageResult {
    pub endpoint_domain: String,
    pub result_status: String,
}
