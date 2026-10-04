//! The seam: somewhere an issue can be created, and the two implementations
//! of it.
//!
//! # Why a trait and not "call `repo_host` from `deliver`"
//!
//! For the reason `mail::MailTransport` gives about itself, and it is the
//! reason that made mail testable: a test has to be able to assert **what
//! would have been sent** without sending it. Delivery is otherwise a function
//! whose only observable behaviour is an HTTP request to github.com, and the
//! rules worth testing — the labels, the body, the search-before-create after
//! an ambiguous failure, the vocabulary a failure is recorded in — are all
//! decided on this side of it.
//!
//! # The GitHub implementation holds the destination
//!
//! One repository per instance (spec.md's Assumptions), so the owner, the name,
//! the installation and the attachment branch are fixed for the life of a
//! host. That keeps every method's signature about the *call* rather than about
//! where it goes, and it means `deliver.rs` never names an installation
//! reference — the same boundary `open_issue_for_connection` keeps for lore
//! sync, and for the same reason.

use async_trait::async_trait;

use super::DestinationRow;
use crate::repo_host::scoped::{self, HostFailure, PostedIssue, ScopedApp};

/// A file committed to the attachment branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attached {
    /// The blob page. Works for a public repository and a private one.
    pub blob_url: String,
    /// The raw URL. Only embedded in a body when the destination is *observed*
    /// to be public — see `contracts/delivery.md` § 3 rule 4.
    pub raw_url: String,
}

/// Somewhere feedback can become an issue.
#[async_trait]
pub trait IssueHost: Send + Sync {
    /// The issue an earlier ambiguous attempt may already have created.
    ///
    /// `Ok(None)` is "the host answered and there is none", which is what lets
    /// a retry create with confidence. An error is "the question could not be
    /// asked", which is a different thing — collapsing them into an `Option`
    /// is how a retry ends up creating a duplicate on a bad day.
    async fn find_by_key(&self, delivery_key: &str) -> Result<Option<PostedIssue>, HostFailure>;

    /// Commit one attachment, returning where it landed.
    async fn attach(&self, path: &str, bytes: &[u8]) -> Result<Attached, HostFailure>;

    async fn create(
        &self,
        title: &str,
        body: &str,
        labels: &[String],
    ) -> Result<PostedIssue, HostFailure>;

    /// `open` or `closed`, as the tracker says today (research § R14).
    async fn state_of(&self, number: i32) -> Result<String, HostFailure>;

    /// Read from the host, never assumed (FR-014, research § R11).
    async fn is_public(&self) -> Result<bool, HostFailure>;
}

/// The real one.
pub struct GitHubIssueHost {
    app: ScopedApp,
    destination: DestinationRow,
}

impl GitHubIssueHost {
    pub fn new(app: ScopedApp, destination: DestinationRow) -> Self {
        GitHubIssueHost { app, destination }
    }

    /// `(owner, name)`, or a refusal the caller records as the host declining.
    ///
    /// A destination whose repository reference is not `owner/name` is
    /// misconfiguration, and reporting it as a 404 puts it in the same bucket
    /// as "the repository is gone" — which is where an operator will look and
    /// is a true description of the situation from here.
    fn repository(&self) -> Result<(&str, &str), HostFailure> {
        self.destination.owner_and_name().ok_or(HostFailure {
            status: Some(404),
            transport: false,
        })
    }

    async fn credential(
        &self,
    ) -> Result<thunderforge_repo_host::RepositoryCredential, HostFailure> {
        scoped::installation_credential(&self.app, &self.destination.installation_ref).await
    }
}

#[async_trait]
impl IssueHost for GitHubIssueHost {
    async fn find_by_key(&self, delivery_key: &str) -> Result<Option<PostedIssue>, HostFailure> {
        let (owner, name) = self.repository()?;
        let credential = self.credential().await?;
        scoped::find_issue_by_key(&self.app, &credential, owner, name, delivery_key).await
    }

    async fn attach(&self, path: &str, bytes: &[u8]) -> Result<Attached, HostFailure> {
        let (owner, name) = self.repository()?;
        let branch = &self.destination.attachment_branch;
        let credential = self.credential().await?;
        let blob_url = scoped::put_file(
            &self.app,
            &credential,
            owner,
            name,
            branch,
            path,
            bytes,
            "Feedback attachment",
        )
        .await?;
        Ok(Attached {
            blob_url,
            raw_url: scoped::raw_url(owner, name, branch, path),
        })
    }

    async fn create(
        &self,
        title: &str,
        body: &str,
        labels: &[String],
    ) -> Result<PostedIssue, HostFailure> {
        let (owner, name) = self.repository()?;
        let credential = self.credential().await?;
        scoped::create_issue(&self.app, &credential, owner, name, title, body, labels).await
    }

    async fn state_of(&self, number: i32) -> Result<String, HostFailure> {
        let (owner, name) = self.repository()?;
        let credential = self.credential().await?;
        scoped::issue_state(&self.app, &credential, owner, name, number).await
    }

    async fn is_public(&self) -> Result<bool, HostFailure> {
        let (owner, name) = self.repository()?;
        let credential = self.credential().await?;
        scoped::repository_is_public(&self.app, &credential, owner, name).await
    }
}

/// The host a test holds instead of GitHub.
///
/// It proves the code *around* the host — that a submission is recorded before
/// anything is called, that an ambiguous failure makes the next attempt search,
/// that the body and labels are what `issue_body` built, that a failure is
/// recorded in the fixed vocabulary. It proves **nothing about GitHub**, which
/// is the honest boundary `mail::capture` draws for itself and is why the
/// end-to-end harness points a real installation at a real repository.
///
/// Gated on `test` or the `test-support` feature, so it cannot be reached from
/// a production build by accident.
#[cfg(any(test, feature = "test-support"))]
pub mod capture {
    use std::sync::Mutex;

    use super::*;

    /// One call, as the test sees it.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Call {
        Searched {
            delivery_key: String,
        },
        Attached {
            path: String,
            bytes: usize,
        },
        Created {
            title: String,
            body: String,
            labels: Vec<String>,
        },
    }

    pub struct CapturingHost {
        calls: Mutex<Vec<Call>>,
        /// What `create` answers. `None` is success.
        create_failure: Option<HostFailure>,
        /// What `find_by_key` answers when it is asked at all.
        existing: Option<PostedIssue>,
        /// Whether the destination reads as public. Fixed rather than
        /// settable: the embed-versus-link decision is driven by the
        /// *destination row's* observed visibility, not by the host, and a
        /// setter here would test the wrong thing.
        public: bool,
    }

    impl Default for CapturingHost {
        fn default() -> Self {
            CapturingHost {
                calls: Mutex::new(Vec::new()),
                create_failure: None,
                existing: None,
                public: true,
            }
        }
    }

    impl CapturingHost {
        /// A host that accepts everything.
        pub fn accepting() -> Self {
            CapturingHost::default()
        }

        /// A host that refuses creation with the failure given, as often as it
        /// is asked — which is what makes a backoff test possible.
        pub fn refusing(failure: HostFailure) -> Self {
            CapturingHost {
                create_failure: Some(failure),
                ..CapturingHost::default()
            }
        }

        /// A host that already has the issue a previous ambiguous attempt made.
        pub fn holding(issue: PostedIssue) -> Self {
            CapturingHost {
                existing: Some(issue),
                ..CapturingHost::default()
            }
        }

        /// Everything asked of this host, in order.
        pub fn calls(&self) -> Vec<Call> {
            self.calls.lock().unwrap_or_else(|e| e.into_inner()).clone()
        }

        /// The issue this host was asked to create, if it was asked.
        pub fn created(&self) -> Option<Call> {
            self.calls()
                .into_iter()
                .find(|c| matches!(c, Call::Created { .. }))
        }

        fn record(&self, call: Call) {
            self.calls
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(call);
        }
    }

    #[async_trait]
    impl IssueHost for CapturingHost {
        async fn find_by_key(
            &self,
            delivery_key: &str,
        ) -> Result<Option<PostedIssue>, HostFailure> {
            self.record(Call::Searched {
                delivery_key: delivery_key.to_string(),
            });
            Ok(self.existing.clone())
        }

        async fn attach(&self, path: &str, bytes: &[u8]) -> Result<Attached, HostFailure> {
            self.record(Call::Attached {
                path: path.to_string(),
                bytes: bytes.len(),
            });
            Ok(Attached {
                blob_url: format!("https://github.com/o/n/blob/feedback-attachments/{path}"),
                raw_url: format!(
                    "https://raw.githubusercontent.com/o/n/feedback-attachments/{path}"
                ),
            })
        }

        async fn create(
            &self,
            title: &str,
            body: &str,
            labels: &[String],
        ) -> Result<PostedIssue, HostFailure> {
            self.record(Call::Created {
                title: title.to_string(),
                body: body.to_string(),
                labels: labels.to_vec(),
            });
            match &self.create_failure {
                Some(failure) => Err(failure.clone()),
                None => Ok(PostedIssue {
                    number: 7,
                    html_url: "https://github.com/o/n/issues/7".to_string(),
                }),
            }
        }

        async fn state_of(&self, _number: i32) -> Result<String, HostFailure> {
            Ok("open".to_string())
        }

        async fn is_public(&self) -> Result<bool, HostFailure> {
            Ok(self.public)
        }
    }
}
