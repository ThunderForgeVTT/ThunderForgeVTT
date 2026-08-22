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
    #[error("STS AssumeRole response was missing credentials")]
    MissingCredentials,
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
}
