//! `GET /api/feedback-assets/{attachment_id}` — the bytes somebody attached to
//! a feedback submission.
//!
//! Declared here without the prefix, like every other asset route, because the
//! router is merged into `api_router` in `src/app/src/main.rs`. A client asks
//! for `/api/feedback-assets/…`; asking for `/feedback-assets/…` in
//! development reaches the SPA's `index.html` instead, since only `/api` and
//! `/assets` are proxied — which looks like a 200 with the wrong body rather
//! than a routing mistake.
//!
//! Spec 037 US2 (T037). Until this existed, `mySubmissions` could tell a person
//! what they had sent and could not show it to them: the attachment rows were
//! written, the objects were in storage, and nothing could read either back.
//!
//! # Who may read one
//!
//! The person who filed the submission, or an administrator. Nobody else, and
//! not by holding the identifier — attachment ids are database keys, not
//! capabilities, and there is no share link for this. An administrator is
//! included because the whole point of the operator queue is triaging what was
//! reported; a report whose screenshot only its author can see is a report an
//! operator cannot act on.
//!
//! # Why an expired attachment is a 404 and not a 410
//!
//! `contracts/attachments.md` gives attachments a retention window, and the
//! sweep sets `attachments_purged_at` and deletes the objects. Once that has
//! happened the bytes are *gone*, and the honest answer to a request for them
//! is that there is nothing there. A 410 would be more precise about history
//! and would also confirm to any caller that something used to exist at that
//! id, which is a small disclosure with no compensating value.
//!
//! # Content type comes from the row, not from the bytes
//!
//! Stored when the attachment was accepted, alongside a `kind` this route does
//! not interpret. Sniffing the object would be a second opinion about what the
//! person approved, and `contracts/attachments.md` is emphatic that what is
//! delivered is what was shown.

use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Router};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth_middleware::AuthenticatedUser;
use crate::schema::{feedback_attachments, feedback_submissions};
use crate::state::AppState;
use crate::storage::rustfs::{RustFsConfig, read_object};

pub fn router() -> Router<AppState> {
    Router::new().route("/feedback-assets/{attachment_id}", get(serve_attachment))
}

/// What the row says about an attachment, for a caller who may or may not be
/// allowed to have it.
struct AttachmentRow {
    storage_path: String,
    content_type: String,
    owner: Uuid,
    purged: bool,
}

async fn load(state: &AppState, attachment_id: Uuid) -> Option<AttachmentRow> {
    let mut conn = state.db_pool.get().ok()?;
    tokio::task::spawn_blocking(move || {
        feedback_attachments::table
            .inner_join(
                feedback_submissions::table
                    .on(feedback_submissions::id.eq(feedback_attachments::submission_id)),
            )
            .filter(feedback_attachments::id.eq(attachment_id))
            .select((
                feedback_attachments::storage_path,
                feedback_attachments::content_type,
                feedback_submissions::user_id,
                feedback_submissions::attachments_purged_at.nullable(),
            ))
            .first::<(String, String, Uuid, Option<chrono::NaiveDateTime>)>(&mut conn)
            .optional()
    })
    .await
    .ok()?
    .ok()?
    .map(
        |(storage_path, content_type, owner, purged_at)| AttachmentRow {
            storage_path,
            content_type,
            owner,
            purged: purged_at.is_some(),
        },
    )
}

async fn serve_attachment(
    State(state): State<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Path(attachment_id): Path<Uuid>,
) -> Response {
    let Some(row) = load(&state, attachment_id).await else {
        return (StatusCode::NOT_FOUND, "attachment not found").into_response();
    };

    // Authorised before anything about the attachment is disclosed — including
    // whether it has been purged. A caller who may not read it learns the same
    // thing either way.
    if row.owner != auth_user.user_id && !auth_user.is_admin {
        return (
            StatusCode::FORBIDDEN,
            "not permitted to read this attachment",
        )
            .into_response();
    }

    if row.purged {
        return (StatusCode::NOT_FOUND, "attachment has expired").into_response();
    }

    let cfg = RustFsConfig::from_env();
    match read_object(&cfg, &row.storage_path).await {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, row.content_type),
                // Never rendered inline by a browser that guessed differently,
                // and never cached by a shared proxy: one of these objects is
                // a screenshot of somebody's session.
                (header::CACHE_CONTROL, "private, no-store".to_string()),
                (
                    header::CONTENT_SECURITY_POLICY,
                    "default-src 'none'; sandbox".to_string(),
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "attachment object not found").into_response(),
    }
}
