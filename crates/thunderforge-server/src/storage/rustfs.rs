//! Spec 002 (FR-017): RustFS S3-compatible object storage client, with
//! per-write STS `AssumeRole` credential minting scoped to exactly the
//! one object key being written. The minted credential is used inside
//! this module only — it is never returned to a caller, let alone a
//! GraphQL client (see `docs/adrs/20260820-039-*.md`, verified against a
//! real running `rustfs/rustfs:1.0.0-rc.2` container before being relied
//! upon here: a policy-scoped `AssumeRole` credential wrote its allowed
//! key, was denied writing any other key, and was denied `ListBuckets`).

use aws_sdk_s3::config::{BehaviorVersion, Credentials as S3Credentials, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_sts::config::Credentials as StsCredentials;
use serde_json::json;
use uuid::Uuid;

use crate::settings::resolver::{Settings, resolve_all};
use crate::state::AppState;

/// STS credential TTL for a single write (research.md §3's "target: 15
/// minutes").
const CREDENTIAL_TTL_SECONDS: i32 = 900;

#[derive(Debug, Clone)]
pub struct RustFsConfig {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub root_access_key: String,
    pub root_secret_key: String,
}

impl RustFsConfig {
    /// The configuration as the registry resolves it: a row written by first-run
    /// setup, overridden by the environment, falling back to the same defaults
    /// `from_env` has always used.
    ///
    /// Pure, and separate from `resolve` for the reason `SmtpTransport` is:
    /// the interesting half is which value wins, and that is worth testing
    /// without a database.
    pub fn from_settings(settings: &Settings) -> Self {
        let value = |key: &str, fallback: &str| -> String {
            settings
                .get(key)
                .and_then(|r| r.value.clone())
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| fallback.to_string())
        };
        Self {
            endpoint: value("storage.endpoint", "http://localhost:9000"),
            region: value("storage.region", "us-east-1"),
            bucket: value("storage.bucket", "thunderforge-canvas-assets"),
            root_access_key: value("storage.access_key", "thunderforge-rustfs-root"),
            root_secret_key: value("storage.secret_key", "thunderforge-rustfs-root-secret"),
        }
    }

    /// The configuration for this instance, right now.
    ///
    /// Resolved per use and never cached, which is what lets an operator
    /// correct an endpoint in the wizard or the admin area and have the next
    /// upload use it. Caching it would also make a process-global out of
    /// something every request already has a cheap path to — a shape that has
    /// caused test flakes in this repository before.
    ///
    /// Infallible on purpose. The only way resolution fails is that the
    /// database is unreachable, and in that case the environment is the best
    /// answer available — which is also exactly what this call site did before
    /// a row could hold the value at all. Returning a `Result` here would make
    /// every asset handler carry an error path for a condition it cannot do
    /// anything about.
    pub async fn resolve(state: &AppState) -> Self {
        match resolve_all(state).await {
            Ok(settings) => Self::from_settings(&settings),
            Err(error) => {
                tracing::warn!(
                    %error,
                    "could not resolve storage settings; falling back to the environment"
                );
                Self::from_env()
            }
        }
    }

    /// The configuration from the environment alone.
    ///
    /// Retained for the two places that run before a resolved setting is
    /// available or wanted: bucket creation at boot, and tests that are not
    /// about resolution.
    pub fn from_env() -> Self {
        Self {
            endpoint: std::env::var("RUSTFS_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:9000".to_string()),
            region: std::env::var("RUSTFS_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
            bucket: std::env::var("RUSTFS_BUCKET")
                .unwrap_or_else(|_| "thunderforge-canvas-assets".to_string()),
            root_access_key: std::env::var("RUSTFS_ROOT_ACCESS_KEY")
                .unwrap_or_else(|_| "thunderforge-rustfs-root".to_string()),
            root_secret_key: std::env::var("RUSTFS_ROOT_SECRET_KEY")
                .unwrap_or_else(|_| "thunderforge-rustfs-root-secret".to_string()),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("STS AssumeRole failed: {0}")]
    AssumeRole(String),
    #[error("S3 PutObject failed: {0}")]
    PutObject(String),
    #[error("S3 GetObject failed: {0}")]
    GetObject(String),
    #[error("S3 CreateBucket failed: {0}")]
    CreateBucket(String),
    #[error("S3 HeadBucket failed: {0}")]
    HealthCheck(String),
    #[error("S3 DeleteObject failed: {0}")]
    DeleteObject(String),
    /// A deletion was asked for outside the one prefix this product may
    /// delete under. Not a host failure — a refusal by this module.
    #[error("refusing to delete {0}: only feedback attachments may be deleted")]
    DeleteRefused(String),
    #[error("STS AssumeRole response was missing credentials")]
    MissingCredentials,
    /// Spec 080 FR-004: the range asked for starts at or past the end.
    #[error("requested range is outside the object")]
    RangeNotSatisfiable,
    /// Spec 080 FR-003: the object is no longer the version the reader
    /// named, so a part of it would splice two versions.
    #[error("object changed since the version asked for")]
    PreconditionFailed,
    #[error("S3 HeadObject failed: {0}")]
    HeadObject(String),
}

/// Derives the storage object key for one asset. Never client-supplied —
/// always computed server-side from the authorized (owner_user_id,
/// world_id, scene_id) triple plus a server-generated asset_id, so a
/// caller cannot choose a path into another campaign's prefix (FR-014,
/// data-model.md's "storage_path MUST be derivable purely from
/// (owner_user_id, world_id, scene_id, id)").
pub fn object_key(
    owner_user_id: Uuid,
    world_id: Uuid,
    scene_id: Option<Uuid>,
    asset_id: Uuid,
) -> String {
    match scene_id {
        Some(scene_id) => format!("{owner_user_id}/{world_id}/{scene_id}/{asset_id}.webp"),
        None => format!("{owner_user_id}/{world_id}/_/{asset_id}.webp"),
    }
}

fn sts_client(cfg: &RustFsConfig) -> aws_sdk_sts::Client {
    let creds = StsCredentials::new(
        &cfg.root_access_key,
        &cfg.root_secret_key,
        None,
        None,
        "rustfs-root",
    );
    let conf = aws_sdk_sts::config::Builder::new()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new(cfg.region.clone()))
        .endpoint_url(&cfg.endpoint)
        .credentials_provider(creds)
        .build();
    aws_sdk_sts::Client::from_conf(conf)
}

fn s3_client_with_credentials(
    cfg: &RustFsConfig,
    access_key_id: String,
    secret_access_key: String,
    session_token: String,
) -> aws_sdk_s3::Client {
    let creds = S3Credentials::new(
        access_key_id,
        secret_access_key,
        Some(session_token),
        None,
        "rustfs-scoped-write",
    );
    let conf = aws_sdk_s3::config::Builder::new()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new(cfg.region.clone()))
        .endpoint_url(&cfg.endpoint)
        .credentials_provider(creds)
        .force_path_style(true)
        .build();
    aws_sdk_s3::Client::from_conf(conf)
}

fn root_s3_client(cfg: &RustFsConfig) -> aws_sdk_s3::Client {
    let creds = S3Credentials::new(
        &cfg.root_access_key,
        &cfg.root_secret_key,
        None,
        None,
        "rustfs-root",
    );
    let conf = aws_sdk_s3::config::Builder::new()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new(cfg.region.clone()))
        .endpoint_url(&cfg.endpoint)
        .credentials_provider(creds)
        .force_path_style(true)
        .build();
    aws_sdk_s3::Client::from_conf(conf)
}

/// Cheap connectivity probe for the `/status` page (FR-020-adjacent) — a
/// `HeadBucket` against the configured bucket using the root credential,
/// never exposed further than this module. Returns an error rather than
/// panicking so the status endpoint can report "down" instead of 500ing.
pub async fn health_check(cfg: &RustFsConfig) -> Result<(), StorageError> {
    let client = root_s3_client(cfg);
    client
        .head_bucket()
        .bucket(&cfg.bucket)
        .send()
        .await
        .map(|_| ())
        .map_err(|e| StorageError::HealthCheck(e.to_string()))
}

/// Idempotent bucket bootstrap for local dev (FR-020) — RustFS has no
/// "create bucket on first boot" server flag, so the server ensures it
/// exists using its own root credential (never exposed further than
/// this module) rather than requiring an out-of-band manual step.
pub async fn ensure_bucket(cfg: &RustFsConfig) -> Result<(), StorageError> {
    let client = root_s3_client(cfg);
    match client.create_bucket().bucket(&cfg.bucket).send().await {
        Ok(_) => Ok(()),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("BucketAlreadyOwnedByYou") || msg.contains("BucketAlreadyExists") {
                Ok(())
            } else {
                Err(StorageError::CreateBucket(msg))
            }
        }
    }
}

/// Builds the inline STS session policy scoping a credential to exactly
/// one `PutObject` on `key` in the configured bucket. Exposed (not just
/// inlined into `write_object`) so T038's regression test can assert on
/// its shape directly.
pub fn scoped_write_policy(bucket: &str, key: &str) -> String {
    json!({
        "Version": "2012-10-17",
        "Statement": [{
            "Effect": "Allow",
            "Action": ["s3:PutObject"],
            "Resource": [format!("arn:aws:s3:::{bucket}/{key}")]
        }]
    })
    .to_string()
}

/// Builds the inline STS session policy scoping a credential to exactly
/// one `GetObject` on `key` — the read-side counterpart of
/// `scoped_write_policy`, same single-key-only rationale.
pub fn scoped_read_policy(bucket: &str, key: &str) -> String {
    json!({
        "Version": "2012-10-17",
        "Statement": [{
            "Effect": "Allow",
            "Action": ["s3:GetObject"],
            "Resource": [format!("arn:aws:s3:::{bucket}/{key}")]
        }]
    })
    .to_string()
}

/// Mints an STS credential scoped to exactly one `GetObject` on `key`,
/// uses it immediately to fetch the object's bytes, then lets the
/// credential fall out of scope — never returned to the caller. This is
/// the read counterpart of `write_object`, used by the authenticated
/// `/canvas-assets/{asset_id}` proxy route (never by a GraphQL client
/// directly — the browser fetches image bytes through that route, not
/// from RustFS itself, so this credential stays exactly as
/// server-side-only as the write one does).
pub async fn read_object(cfg: &RustFsConfig, key: &str) -> Result<Vec<u8>, StorageError> {
    let s3 = scoped_reader(cfg, key).await?;

    let output = s3
        .get_object()
        .bucket(&cfg.bucket)
        .key(key)
        .send()
        .await
        .map_err(|e| StorageError::GetObject(e.to_string()))?;

    let bytes = output
        .body
        .collect()
        .await
        .map_err(|e| StorageError::GetObject(e.to_string()))?
        .into_bytes()
        .to_vec();

    Ok(bytes)
}

/// One object opened for reading, its body not yet read.
///
/// Spec 080: the body is a stream, so a route hands it to the client as
/// storage sends it and never holds the object (FR-006).
pub struct OpenObject {
    pub body: ByteStream,
    pub content_length: Option<i64>,
    /// `bytes a-b/size` when a range was asked for and served.
    pub content_range: Option<String>,
    pub e_tag: Option<String>,
    /// An HTTP date.
    pub last_modified: Option<String>,
}

/// Opens `key` for reading, or one byte range of it.
///
/// `range` is an HTTP range spec the caller has already checked is one
/// `bytes=` range. `if_match` is the entity tag the reader expects: storage
/// refuses with [`StorageError::PreconditionFailed`] when the object is no
/// longer that version. A range past the end is
/// [`StorageError::RangeNotSatisfiable`].
///
/// The credential is minted per call, scoped to this one key, exactly as
/// [`read_object`] mints it: a part is a request like any other (FR-005).
pub async fn open_object(
    cfg: &RustFsConfig,
    key: &str,
    range: Option<&str>,
    if_match: Option<&str>,
) -> Result<OpenObject, StorageError> {
    let s3 = scoped_reader(cfg, key).await?;

    let output = s3
        .get_object()
        .bucket(&cfg.bucket)
        .key(key)
        .set_range(range.map(str::to_string))
        .set_if_match(if_match.map(str::to_string))
        .send()
        .await
        .map_err(|e| match e.raw_response().map(|r| r.status().as_u16()) {
            Some(416) => StorageError::RangeNotSatisfiable,
            Some(412) => StorageError::PreconditionFailed,
            _ => StorageError::GetObject(e.to_string()),
        })?;

    Ok(OpenObject {
        content_length: output.content_length(),
        content_range: output.content_range().map(str::to_string),
        e_tag: output.e_tag().map(str::to_string),
        last_modified: output
            .last_modified()
            .and_then(|t| t.fmt(aws_sdk_s3::primitives::DateTimeFormat::HttpDate).ok()),
        body: output.body,
    })
}

/// The size of `key` in bytes, for a `416` answer's `bytes */size`.
///
/// `HeadObject` is authorised by the same `s3:GetObject` the scoped read
/// policy grants.
pub async fn object_size(cfg: &RustFsConfig, key: &str) -> Result<i64, StorageError> {
    let s3 = scoped_reader(cfg, key).await?;
    let output = s3
        .head_object()
        .bucket(&cfg.bucket)
        .key(key)
        .send()
        .await
        .map_err(|e| StorageError::HeadObject(e.to_string()))?;
    output
        .content_length()
        .ok_or_else(|| StorageError::HeadObject("no content length".to_string()))
}

/// An S3 client holding a fresh credential that may read `key` and nothing
/// else. It stays inside this module.
async fn scoped_reader(cfg: &RustFsConfig, key: &str) -> Result<aws_sdk_s3::Client, StorageError> {
    let sts = sts_client(cfg);
    let policy = scoped_read_policy(&cfg.bucket, key);

    let assumed = sts
        .assume_role()
        .role_arn("arn:aws:iam::000000000000:role/thunderforge-canvas-asset-reader")
        .role_session_name(format!("read-{}", Uuid::now_v7()))
        .policy(policy)
        .duration_seconds(CREDENTIAL_TTL_SECONDS)
        .send()
        .await
        .map_err(|e| StorageError::AssumeRole(e.to_string()))?;

    let creds = assumed
        .credentials()
        .ok_or(StorageError::MissingCredentials)?;
    let s3 = s3_client_with_credentials(
        cfg,
        creds.access_key_id().to_string(),
        creds.secret_access_key().to_string(),
        creds.session_token().to_string(),
    );
    Ok(s3)
}

/// Mints an STS credential scoped to exactly one `PutObject` on `key`,
/// uses it immediately to write `bytes`, then lets the credential fall
/// out of scope when this function returns — it is never returned to
/// the caller. Returns the object key on success (the caller already
/// knows it, but returning it keeps the call site's `let storage_path =
/// write_object(...).await?;` reading naturally).
pub async fn write_object(
    cfg: &RustFsConfig,
    key: &str,
    bytes: Vec<u8>,
    content_type: &str,
) -> Result<String, StorageError> {
    let sts = sts_client(cfg);
    let policy = scoped_write_policy(&cfg.bucket, key);

    let assumed = sts
        .assume_role()
        .role_arn("arn:aws:iam::000000000000:role/thunderforge-canvas-asset-writer")
        .role_session_name(format!("write-{}", Uuid::now_v7()))
        .policy(policy)
        .duration_seconds(CREDENTIAL_TTL_SECONDS)
        .send()
        .await
        .map_err(|e| StorageError::AssumeRole(e.to_string()))?;

    let creds = assumed
        .credentials()
        .ok_or(StorageError::MissingCredentials)?;
    let s3 = s3_client_with_credentials(
        cfg,
        creds.access_key_id().to_string(),
        creds.secret_access_key().to_string(),
        creds.session_token().to_string(),
    );

    s3.put_object()
        .bucket(&cfg.bucket)
        .key(key)
        .body(ByteStream::from(bytes))
        .content_type(content_type)
        .send()
        .await
        .map_err(|e| StorageError::PutObject(e.to_string()))?;

    Ok(key.to_string())
}

/// The **only** deletion path in this codebase, and it refuses every key that
/// is not a feedback attachment.
///
/// # Why the restriction is inside the function
///
/// `storage/dedupe.rs` states the rule this is bounded by: "Nothing in this
/// product deletes stored objects… which is what makes a shared path safe
/// today: a reference cannot dangle when references are never dropped… Adding
/// object deletion means adding reference counting first." Feedback
/// attachments are the one case that needs no reference counting, because they
/// are **never deduplicated** — one row, one object, no other referrer, and
/// `feedback_attachments` deliberately has no `content_hash` column by which a
/// shared path could be introduced.
///
/// So the prefix check lives here rather than in the callers, because
/// `dedupe.rs` warns that deleting a shared object "would silently blank the
/// background of every other scene sharing those bytes" — and a rule kept by
/// callers is a rule until somebody adds a caller.
///
/// Spec 037 FR-016: retention has to be bounded, and bounded retention means
/// deletion.
///
/// Spec 048 FR-031: an uploaded character sheet is the second such case. A
/// player may delete their brought character, and its files go with it; a
/// sheet is one row, one object, and never deduplicated either.
pub async fn delete_object(cfg: &RustFsConfig, key: &str) -> Result<(), StorageError> {
    if !DELETABLE_PREFIXES
        .iter()
        .any(|prefix| key.starts_with(prefix))
    {
        return Err(StorageError::DeleteRefused(key.to_string()));
    }

    root_s3_client(cfg)
        .delete_object()
        .bucket(&cfg.bucket)
        .key(key)
        .send()
        .await
        .map_err(|e| StorageError::DeleteObject(e.to_string()))?;

    Ok(())
}

/// The one prefix `delete_object` will touch. Duplicated from
/// `feedback::STORAGE_PREFIX` on purpose: this module must not depend on a
/// feature module for a safety rule about its own bucket, and the test below
/// pins the two together.
pub const FEEDBACK_PREFIX: &str = "feedback/";

/// Uploaded character sheets (spec 048). Duplicated from
/// `sheet_import::storage::STORAGE_PREFIX` for the same reason, and pinned the
/// same way.
pub const SHEETS_PREFIX: &str = "sheets/";

/// Every prefix `delete_object` will touch, and no other.
pub const DELETABLE_PREFIXES: &[&str] = &[FEEDBACK_PREFIX, SHEETS_PREFIX];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_key_is_derived_not_free_form() {
        let owner = Uuid::nil();
        let world = Uuid::nil();
        let scene = Some(Uuid::nil());
        let asset = Uuid::nil();
        let key = object_key(owner, world, scene, asset);
        assert_eq!(
            key,
            "00000000-0000-0000-0000-000000000000/00000000-0000-0000-0000-000000000000/00000000-0000-0000-0000-000000000000/00000000-0000-0000-0000-000000000000.webp"
        );
    }

    /// T038: regression fixture for the write-credential session policy
    /// — single-key scoped, PutObject only, no wildcard.
    #[test]
    fn scoped_write_policy_names_exactly_one_key() {
        let policy_json = scoped_write_policy("my-bucket", "a/b/c/d.webp");
        let parsed: serde_json::Value = serde_json::from_str(&policy_json).unwrap();
        let resources = parsed["Statement"][0]["Resource"].as_array().unwrap();
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0], "arn:aws:s3:::my-bucket/a/b/c/d.webp");
        assert!(!resources[0].as_str().unwrap().contains('*'));
        let actions = parsed["Statement"][0]["Action"].as_array().unwrap();
        assert_eq!(actions, &vec![serde_json::json!("s3:PutObject")]);
    }

    /// Read-side counterpart of `scoped_write_policy_names_exactly_one_key`.
    #[test]
    fn scoped_read_policy_names_exactly_one_key() {
        let policy_json = scoped_read_policy("my-bucket", "a/b/c/d.webp");
        let parsed: serde_json::Value = serde_json::from_str(&policy_json).unwrap();
        let resources = parsed["Statement"][0]["Resource"].as_array().unwrap();
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0], "arn:aws:s3:::my-bucket/a/b/c/d.webp");
        assert!(!resources[0].as_str().unwrap().contains('*'));
        let actions = parsed["Statement"][0]["Action"].as_array().unwrap();
        assert_eq!(actions, &vec![serde_json::json!("s3:GetObject")]);
    }

    /// The restriction that keeps `dedupe.rs`'s warning true. A key outside
    /// the feedback prefix names an object that may be shared by rows nothing
    /// counts, and deleting one would blank a scene nobody touched.
    #[tokio::test]
    async fn delete_object_refuses_every_key_outside_the_feedback_prefix() {
        let cfg = RustFsConfig {
            endpoint: "http://127.0.0.1:1".to_string(),
            region: "us-east-1".to_string(),
            bucket: "b".to_string(),
            root_access_key: "k".to_string(),
            root_secret_key: "s".to_string(),
        };
        // A canvas asset key: owner/world/scene/asset.webp. No network call is
        // made, which is the point — the refusal happens before the client is
        // built, so a bug here cannot become a deletion that "only" failed to
        // connect.
        let refused = delete_object(&cfg, "a/b/c/d.webp").await;
        assert!(matches!(refused, Err(StorageError::DeleteRefused(_))));
        let refused = delete_object(&cfg, "notfeedback/x.webp").await;
        assert!(matches!(refused, Err(StorageError::DeleteRefused(_))));
    }

    /// The prefix this module enforces and the prefix the feedback module
    /// writes under are one string. Two would drift, and the drift would be
    /// invisible until a sweep silently deleted nothing.
    #[test]
    fn the_deletable_prefix_is_the_one_feedback_writes_under() {
        assert_eq!(FEEDBACK_PREFIX, crate::feedback::STORAGE_PREFIX);
    }

    #[test]
    fn the_sheets_prefix_is_the_one_sheet_import_writes_under() {
        assert_eq!(SHEETS_PREFIX, crate::sheet_import::storage::STORAGE_PREFIX);
    }

    /// Spec 048 T022: a sheet's key passes the prefix check, so the refusal
    /// is not what stops it. The endpoint is unreachable, so the call fails
    /// connecting instead, which is the proof that it got past the guard.
    #[tokio::test]
    async fn delete_object_accepts_a_sheet_key_and_still_refuses_others() {
        let cfg = RustFsConfig {
            endpoint: "http://127.0.0.1:1".to_string(),
            region: "us-east-1".to_string(),
            bucket: "b".to_string(),
            root_access_key: "k".to_string(),
            root_secret_key: "s".to_string(),
        };
        let key = crate::sheet_import::storage::object_key(Uuid::nil(), Uuid::nil(), 1);
        let attempted = delete_object(&cfg, &key).await;
        assert!(
            matches!(attempted, Err(StorageError::DeleteObject(_))),
            "{attempted:?}"
        );
        for refused in ["sheetsx/a.pdf", "a/sheets/b.pdf", "Sheets/a.pdf"] {
            let refused = delete_object(&cfg, refused).await;
            assert!(matches!(refused, Err(StorageError::DeleteRefused(_))));
        }
    }
}
