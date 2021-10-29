use std::io::Cursor;

use crate::model::Subscription;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use web_push::{
    ContentEncoding, SubscriptionInfo, VapidSignatureBuilder, WebPushClient, WebPushMessageBuilder,
};

#[derive(Serialize, PartialEq, Debug)]
pub struct MessagePayloadData {
    /// URL to open when notification is clicked.
    action: String,
}

#[derive(Serialize, PartialEq, Debug)]
pub struct MessagePayload {
    pub message: String,
    vibrate: bool,
    silent: bool,
    channel: String,
    data: MessagePayloadData,
}

#[derive(Deserialize)]
struct MessageFormData {
    message: String,
    action: Option<String>,
}

impl MessagePayload {
    pub fn parse_new(message: &str, channel: &str, default_action: &str) -> Self {
        let message = match serde_urlencoded::from_str::<MessageFormData>(message) {
            Ok(message) => message,
            Err(_) => MessageFormData {
                message: message.to_string(),
                action: None,
            },
        };

        MessagePayload {
            message: message.message,
            channel: channel.to_string(),
            silent: false,
            vibrate: false,
            data: MessagePayloadData {
                action: message.action.unwrap_or_else(|| default_action.to_string()),
            },
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_parse_plain_message() {
        let payload = MessagePayload::parse_new("my message", "abcdef", "http://blah/c/abcdef");

        assert_eq!(
            MessagePayload {
                message: "my message".to_string(),
                channel: "abcdef".to_string(),
                vibrate: false,
                silent: false,
                data: MessagePayloadData {
                    action: "http://blah/c/abcdef".to_string()
                }
            },
            payload
        );
    }

    #[test]
    pub fn test_parse_message() {
        let payload = MessagePayload::parse_new(
            "message=this+is+my+message",
            "abcdef",
            "http://blah/c/abcdef",
        );

        assert_eq!(
            MessagePayload {
                message: "this is my message".to_string(),
                channel: "abcdef".to_string(),
                vibrate: false,
                silent: false,
                data: MessagePayloadData {
                    action: "http://blah/c/abcdef".to_string()
                }
            },
            payload
        );
    }

    #[test]
    pub fn test_parse_message_with_action() {
        let payload = MessagePayload::parse_new(
            "message=this+is+my+message&action=https://www.example.com/",
            "abcdef",
            "http://blah/c/abcdef",
        );

        assert_eq!(
            MessagePayload {
                message: "this is my message".to_string(),
                channel: "abcdef".to_string(),
                vibrate: false,
                silent: false,
                data: MessagePayloadData {
                    action: "https://www.example.com/".to_string()
                }
            },
            payload
        );
    }
}
