//! The hop to the public collector (FR-044): one client, a bounded number of
//! posts in flight, and a request built fresh with no header but
//! `Content-Type`.

use std::sync::Arc;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::config::{Config, UPSTREAM_CONNECT_TIMEOUT};
use crate::intake::Signal;

pub struct Upstream {
    client: reqwest::Client,
    base: String,
    slots: Arc<Semaphore>,
}

impl Upstream {
    pub fn new(config: &Config) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .connect_timeout(UPSTREAM_CONNECT_TIMEOUT)
            .timeout(config.upstream_timeout())
            .build()?;
        Ok(Self {
            client,
            base: config.upstream.trim_end_matches('/').to_owned(),
            slots: Arc::new(Semaphore::new(config.upstream_in_flight.max(1))),
        })
    }

    /// A slot, or `None` at once when every one is taken.
    pub fn try_slot(&self) -> Option<OwnedSemaphorePermit> {
        self.slots.clone().try_acquire_owned().ok()
    }

    /// Posts a protobuf body. The collector's answer body on a 2xx, or
    /// `None` on any error, non-2xx or timeout.
    pub async fn post(
        &self,
        _slot: OwnedSemaphorePermit,
        signal: Signal,
        body: Vec<u8>,
    ) -> Option<Vec<u8>> {
        let url = format!("{}/v1/{}", self.base, signal.as_str());
        let response = self
            .client
            .post(url)
            .header("content-type", "application/x-protobuf")
            .body(body)
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        response.bytes().await.ok().map(|b| b.to_vec())
    }
}
