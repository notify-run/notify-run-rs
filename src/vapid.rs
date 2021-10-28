use std::io::Cursor;

use crate::model::Subscription;
use anyhow::Result;
use serde::Serialize;
use web_push::{
    ContentEncoding, SubscriptionInfo, VapidSignatureBuilder, WebPushClient, WebPushMessageBuilder,
};

#[derive(Serialize)]
pub struct MessagePayload {
    message: String,
    vibrate: bool,
    silent: bool,
    channel: String,
}

impl MessagePayload {
    pub fn new(message: &str, channel: &str) -> Self {
        MessagePayload {
            message: message.to_string(),
            channel: channel.to_string(),
            silent: false,
            vibrate: false,
        }
    }
}

pub async fn send_message(
    message: &MessagePayload,
    subscription: &Subscription,
    vapid_privkey: &[u8],
) -> Result<()> {
    let subscription_info = SubscriptionInfo::new(
        subscription.endpoint.clone(),
        subscription.p256dh.clone(),
        subscription.auth.clone(),
    );

    let cursor = Cursor::new(&vapid_privkey);
    let sig_builder = VapidSignatureBuilder::from_der_no_sub(cursor)?;

    let signature = sig_builder
        .add_sub_info(&subscription_info)
        .build()
        .unwrap();

    let mut builder = WebPushMessageBuilder::new(&subscription_info)?;
    let payload_json = serde_json::to_string(message)?;
    builder.set_payload(ContentEncoding::Aes128Gcm, payload_json.as_bytes());
    builder.set_vapid_signature(signature);

    let client = WebPushClient::new()?;
    client.send(builder.build()?).await?;

    Ok(())
}
