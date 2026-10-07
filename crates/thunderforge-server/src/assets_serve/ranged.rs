//! Spec 080: answering a request for a stored object, whole or in part.
//!
//! Every route in this directory decides permission first and only then
//! calls [`serve`]. Nothing here knows who is asking; a caller that got this
//! far is allowed the whole file, so it is allowed any part of it, and a
//! caller that was refused never reaches this module (FR-005).
//!
//! The answer follows `contracts/http-ranges.md`:
//!
//! - no `Range`, several ranges, or one that does not parse: `200`, whole;
//! - one `bytes=` range inside the file: `206` with `Content-Range`;
//! - `If-Range` naming another version: `200` with the whole current file;
//! - a range starting past the end: `416` with `bytes */size`.
//!
//! Every success carries `Accept-Ranges: bytes` and the object's `ETag`, and
//! its body is the storage stream, never the object held in memory (FR-006).

use axum::body::Body;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures_util::stream;

use crate::storage::rustfs::{OpenObject, RustFsConfig, StorageError, object_size, open_object};

/// One `bytes=` range spec, as storage takes it, or `None` for the whole
/// file.
///
/// Accepts `bytes=a-b` (with `a <= b`), `bytes=a-` and `bytes=-n` (`n > 0`).
/// Several ranges are answered whole (FR-007); anything that does not parse
/// is ignored, as HTTP says a server should.
pub(crate) fn wanted_range(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::RANGE)?.to_str().ok()?.trim();
    let spec = value.strip_prefix("bytes=")?.trim();
    if spec.contains(',') {
        return None;
    }
    let (start, end) = spec.split_once('-')?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    match (start.is_empty(), end.is_empty()) {
        (false, true) if digits(start) => {}
        (true, false) if digits(end) && end.parse::<u64>().ok()? > 0 => {}
        (false, false) if digits(start) && digits(end) => {
            if start.parse::<u64>().ok()? > end.parse::<u64>().ok()? {
                return None;
            }
        }
        _ => return None,
    }
    Some(format!("bytes={spec}"))
}

/// What an `If-Range` header asks.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum IfRange {
    /// No `If-Range`: the range stands.
    Absent,
    /// A strong entity tag: the range stands only while the object is that
    /// version.
    Tag(String),
    /// A date, a weak tag or something unreadable. A date cannot be
    /// compared strongly against storage, and a weak tag must never be used
    /// for a range, so the whole file is sent.
    Distrust,
}

pub(crate) fn if_range(headers: &HeaderMap) -> IfRange {
    let Some(value) = headers.get(header::IF_RANGE) else {
        return IfRange::Absent;
    };
    match value.to_str().map(str::trim) {
        Ok(tag) if tag.starts_with('"') && tag.ends_with('"') && tag.len() >= 2 => {
            IfRange::Tag(tag.to_string())
        }
        _ => IfRange::Distrust,
    }
}

/// Serves `key`, whole or one range of it, with the route's own headers
/// (`Content-Type`, `Cache-Control`) added.
///
/// A storage failure is returned to the route, which answers it as it
/// always has.
pub async fn serve(
    cfg: &RustFsConfig,
    key: &str,
    headers: &HeaderMap,
    extra: &[(HeaderName, &'static str)],
) -> Result<Response, StorageError> {
    let range = match (wanted_range(headers), if_range(headers)) {
        (None, _) | (Some(_), IfRange::Distrust) => None,
        (Some(range), IfRange::Absent) => Some((range, None)),
        (Some(range), IfRange::Tag(tag)) => Some((range, Some(tag))),
    };

    let opened = match &range {
        None => open_object(cfg, key, None, None).await,
        Some((range, tag)) => open_object(cfg, key, Some(range), tag.as_deref()).await,
    };
    let object = match opened {
        Ok(object) => object,
        // The version changed under the reader: the whole current file, so
        // a resume never splices two versions (FR-003).
        Err(StorageError::PreconditionFailed) => open_object(cfg, key, None, None).await?,
        Err(StorageError::RangeNotSatisfiable) => {
            let size = object_size(cfg, key).await?;
            return Ok(not_satisfiable(size, extra));
        }
        Err(other) => return Err(other),
    };
    Ok(answer(object, extra))
}

/// `416` with the object's real size and no bytes (FR-004).
pub(crate) fn not_satisfiable(size: i64, extra: &[(HeaderName, &'static str)]) -> Response {
    let mut response = StatusCode::RANGE_NOT_SATISFIABLE.into_response();
    let h = response.headers_mut();
    for (name, value) in extra {
        h.insert(name.clone(), HeaderValue::from_static(value));
    }
    h.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    if let Ok(v) = HeaderValue::from_str(&format!("bytes */{size}")) {
        h.insert(header::CONTENT_RANGE, v);
    }
    response
}

/// `200` or `206`, its body the storage stream.
fn answer(object: OpenObject, extra: &[(HeaderName, &'static str)]) -> Response {
    let OpenObject {
        body,
        content_length,
        content_range,
        e_tag,
        last_modified,
    } = object;

    let status = if content_range.is_some() {
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };

    // One chunk at a time from storage; the stream ends after the first
    // error so a broken read cannot spin.
    let chunks = stream::unfold(Some(body), |state| async move {
        let mut body = state?;
        match body.next().await? {
            Ok(bytes) => Some((Ok(bytes), Some(body))),
            Err(error) => Some((Err(std::io::Error::other(error)), None)),
        }
    });

    let mut response = Response::new(Body::from_stream(chunks));
    *response.status_mut() = status;
    let h = response.headers_mut();
    for (name, value) in extra {
        h.insert(name.clone(), HeaderValue::from_static(value));
    }
    h.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    let mut put = |name: HeaderName, value: Option<String>| {
        if let Some(v) = value.and_then(|v| HeaderValue::from_str(&v).ok()) {
            h.insert(name, v);
        }
    };
    put(
        header::CONTENT_LENGTH,
        content_length.map(|n| n.to_string()),
    );
    put(header::CONTENT_RANGE, content_range);
    put(header::ETAG, e_tag);
    put(header::LAST_MODIFIED, last_modified);
    response
}

#[cfg(test)]
#[path = "ranged_tests.rs"]
mod tests;
