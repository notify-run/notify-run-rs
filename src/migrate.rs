use anyhow::Result;
use chrono::{NaiveDateTime, TimeZone, Utc};
use serde::Deserialize;
use std::{collections::HashMap, fs::read_to_string, path::PathBuf};
use tiny_firestore_odm::{Collection, Database};

use crate::model::Channel;

#[derive(Deserialize)]
struct DynamoExport {
    #[serde(rename = "Items")]
    items: Vec<Item>,
}

#[derive(Deserialize)]
struct DynamoString {
    #[serde(rename = "S")]
    value: String,
}

#[derive(Deserialize)]
struct DynamoValue<T> {
    #[serde(rename = "M")]
    value: T,
}

#[derive(Deserialize)]
struct Keys {
    auth: DynamoString,
    p256dh: DynamoString,
}

#[derive(Deserialize)]
struct Subscription {
    endpoint: DynamoString,
    keys: DynamoValue<Keys>,
}

#[derive(Deserialize)]
struct ChannelMeta {
    agent: DynamoString,
    ip: DynamoString,
}

#[derive(Deserialize)]
struct Item {
    created: DynamoString,
    #[serde(rename = "channelId")]
    channel_id: DynamoString,
    subscriptions: DynamoValue<HashMap<String, DynamoValue<Subscription>>>,
    meta: DynamoValue<ChannelMeta>,
}

pub async fn migrate(path: PathBuf, db: Database) -> Result<()> {
    let migrate_json = read_to_string(path)?;

    let channels: Collection<Channel> = db.collection("channels");

    let migrate: DynamoExport = serde_json::from_str(&migrate_json)?;

    for (index, item) in migrate.items.into_iter().enumerate() {
        let channel_id = item.channel_id.value;
        let _span = tracing::info_span!("Channel", %channel_id).entered();
        let created_naive =
            NaiveDateTime::parse_from_str(&item.created.value, "%Y-%m-%d %H:%M:%S.%f")?;
        let created = Utc.from_utc_datetime(&created_naive);

        let channel = Channel {
            created,
            created_agent: item.meta.value.agent.value,
            created_ip: item.meta.value.ip.value,
        };

        tracing::info!(%index, "Inserting channel.");
        let created = channels.try_create(&channel, &*channel_id).await?;
        tracing::info!(%created, "Success (channel).");

        for (subscription_id, subscription) in item.subscriptions.value {
            let _span = tracing::info_span!("Subscription", %subscription_id).entered();
            let subscriptions: Collection<crate::model::Subscription> =
                channels.subcollection(&channel_id, "susbcriptions");

            let sub = crate::model::Subscription {
                endpoint: subscription.value.endpoint.value,
                auth: subscription.value.keys.value.auth.value,
                p256dh: subscription.value.keys.value.p256dh.value,
            };

            tracing::info!("Inserting subscription.");
            let created = subscriptions.try_create(&sub, &*subscription_id).await?;
            tracing::info!(%created, "Success (subscription).");
        }
    }

    Ok(())
}
