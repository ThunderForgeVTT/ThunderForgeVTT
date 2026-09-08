//! The transport a test holds instead of a mail server.
//!
//! # What it is for, and what it is not for
//!
//! It proves the code *around* the transport: that an enqueued message reaches
//! one, that a refusal is recorded rather than lost, that a body is never
//! logged, that the outbox transitions. It proves **nothing at all about
//! SMTP** — that is what the Mailpit-backed integration level is for
//! (research.md § R8), and a subsystem that has only ever talked to this is a
//! subsystem that has never sent an email.
//!
//! Gated on `test` or the `test-support` feature, the same way `test_support`
//! itself is, so it cannot be reached from a production build by accident.

#![cfg(any(test, feature = "test-support"))]

use std::sync::Mutex;

use super::{Availability, DeliveryFailure, MailTransport, OutgoingMessage};

/// Collects what would have been sent.
pub struct CapturingTransport {
    sent: Mutex<Vec<OutgoingMessage>>,
    /// What `send` answers. `None` is success; a failure is returned as often
    /// as it is asked for, which is what makes a backoff test possible.
    outcome: Option<DeliveryFailure>,
    availability: Availability,
}

impl Default for CapturingTransport {
    fn default() -> Self {
        CapturingTransport {
            sent: Mutex::new(Vec::new()),
            outcome: None,
            availability: Availability::Ready,
        }
    }
}

impl CapturingTransport {
    /// A transport that accepts everything.
    pub fn accepting() -> Self {
        CapturingTransport::default()
    }

    /// A transport that refuses everything, with the failure given.
    pub fn refusing(failure: DeliveryFailure) -> Self {
        CapturingTransport {
            outcome: Some(failure),
            ..CapturingTransport::default()
        }
    }

    /// A transport that reports itself unavailable, for asserting what an
    /// instance with no mail configured does with a message.
    pub fn unconfigured(missing: Vec<&'static str>) -> Self {
        CapturingTransport {
            outcome: Some(DeliveryFailure::permanent(
                super::Unconfigured::new(missing.clone()).reason(),
            )),
            availability: Availability::Unconfigured { missing },
            ..CapturingTransport::default()
        }
    }

    /// Everything handed to this transport, in order. Includes messages a
    /// refusing transport refused — "what would have been sent" is the
    /// question, and a refusal does not unask it.
    pub fn captured(&self) -> Vec<OutgoingMessage> {
        self.sent.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

#[async_trait::async_trait]
impl MailTransport for CapturingTransport {
    async fn send(&self, message: &OutgoingMessage) -> Result<(), DeliveryFailure> {
        self.sent
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(message.clone());
        match &self.outcome {
            None => Ok(()),
            Some(failure) => Err(failure.clone()),
        }
    }

    fn availability(&self) -> Availability {
        self.availability.clone()
    }
}
