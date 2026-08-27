//! [`BlobStore`] against the real Origin Private File System.
//!
//! Spec `028-client-world-cache` (T024, T055). Moved here from
//! `thunderforge-cache-browser` so it sits behind the trait its in-memory
//! twin also implements.
//!
//! # The one thing this file has to get right
//!
//! Creating a file and filling it are two steps, and the gap between them is
//! visible to every other tab. Per the WHATWG File System Standard,
//! `getFileHandle(name, {create: true})` sets the new entry's binary data to
//! an empty byte sequence and appends it to the directory *before its promise
//! resolves*; nothing hides it until first write. `createWritable()` then
//! buffers into a swap file and `close()` replaces the entry's data wholesale.
//!
//! So the only intermediate state a reader can ever observe is **empty** —
//! never a prefix. That is what [`BlobShape`] keys off, and why the rule is
//! "an empty file is not something we finished writing" rather than a guess
//! at a minimum length.
//!
//! Before that rule, a reader that found an empty file concluded the content
//! would not decrypt and **deleted it** — reclaiming, with no lock held, a
//! file another tab was in the middle of writing. The write then completed
//! into a removed entry, and the index row that followed pointed at nothing.
//!
//! There is no portable way to avoid the window itself: `move()` (write to a
//! temp name, rename into place) is in no specification, is unbound in
//! web-sys 0.3, and in Chrome exists only on file handles. So the window is
//! left where the platform puts it and made harmless on the read side.
//!
//! # Two tabs writing one blob
//!
//! Allowed, and not defended against here. `createWritable` takes a *shared*
//! lock, so both writes succeed and the last `close` wins — and in Firefox
//! and Safari there is no exclusion available at all. It is safe for a reason
//! peculiar to this cache: the filename is the fingerprint of the content, so
//! two tabs writing the same name are writing identical bytes, and
//! last-write-wins between identical writes is not a lost update. Ordering
//! against *eviction* is a different question, and that is what the callers'
//! Web Lock is for.

use js_sys::Uint8Array;
use thunderforge_cache_core::Fingerprint;
use uuid::Uuid;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemGetDirectoryOptions,
    FileSystemGetFileOptions, FileSystemRemoveOptions, FileSystemWritableFileStream,
    StorageManager,
};

use crate::paths::{UserScope, blob_file_name, fingerprint_from_file_name, world_dir_name};
use crate::store::{BlobShape, BlobStore, Result, StoreError};

fn js_err(err: JsValue) -> StoreError {
    StoreError::Backend(
        js_sys::Reflect::get(&err, &JsValue::from_str("message"))
            .ok()
            .and_then(|m| m.as_string())
            .or_else(|| err.as_string())
            .unwrap_or_else(|| "unknown".to_string()),
    )
}

/// Read a property off `globalThis`.
fn global_property(name: &str) -> Result<JsValue> {
    js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str(name))
        .map_err(|_| StoreError::Unsupported("globalThis"))
}

/// The blob store rooted at one user's OPFS scope.
///
/// The scope is fixed when the store is opened, so no call can reach another
/// user's bytes by passing the wrong argument.
pub struct OpfsBlobStore {
    scope_root: FileSystemDirectoryHandle,
    scope: UserScope,
}

impl OpfsBlobStore {
    /// Open (creating if absent) the scope directory for this user.
    pub async fn open(scope: UserScope) -> Result<Self> {
        let navigator = global_property("navigator")?;
        let storage = js_sys::Reflect::get(&navigator, &JsValue::from_str("storage"))
            .map_err(|_| StoreError::Unsupported("OPFS (navigator.storage)"))?;
        if storage.is_undefined() || storage.is_null() {
            return Err(StoreError::Unsupported("OPFS (navigator.storage)"));
        }
        let storage: StorageManager = storage.unchecked_into();
        let root = JsFuture::from(storage.get_directory())
            .await
            .map_err(|_| StoreError::Unsupported("OPFS (getDirectory)"))?;
        let root: FileSystemDirectoryHandle = root.unchecked_into();
        let scope_root = get_dir(&root, scope.as_str(), true)
            .await?
            .ok_or(StoreError::Unsupported("OPFS directory creation"))?;
        Ok(Self { scope_root, scope })
    }

    /// The scope this store is confined to.
    pub fn scope(&self) -> &UserScope {
        &self.scope
    }

    /// The `File` snapshot at a name, or `None` if there is no entry.
    ///
    /// Returns the `File` rather than the handle, and callers take the length
    /// from it, because `getFile()` is what costs something here: it is a
    /// round trip that produces a snapshot of the entry. Asking once and
    /// reading both the size and the bytes off the same snapshot also removes
    /// a race this code would otherwise have with itself — between a
    /// "how big is it" call and a separate "give me the bytes" call, another
    /// tab can commit, and the two answers would describe different files.
    async fn snapshot(
        &self,
        world_id: Uuid,
        fingerprint: &Fingerprint,
    ) -> Result<Option<web_sys::File>> {
        let Some(world) = get_dir(&self.scope_root, &world_dir_name(world_id), false).await? else {
            return Ok(None);
        };
        let Some(file) = get_file(&world, &blob_file_name(fingerprint), false).await? else {
            return Ok(None);
        };
        let blob = JsFuture::from(file.get_file()).await.map_err(js_err)?;
        Ok(Some(blob.unchecked_into()))
    }
}

impl BlobStore for OpfsBlobStore {
    async fn write(&self, world_id: Uuid, fingerprint: &Fingerprint, sealed: &[u8]) -> Result<()> {
        let world = get_dir(&self.scope_root, &world_dir_name(world_id), true)
            .await?
            .ok_or(StoreError::Unsupported("OPFS directory creation"))?;
        // This is the call that publishes a zero-length entry to every other
        // tab. Everything after it is invisible to them until `close()`.
        let file = get_file(&world, &blob_file_name(fingerprint), true)
            .await?
            .ok_or(StoreError::Unsupported("OPFS file creation"))?;

        let writable = JsFuture::from(file.create_writable())
            .await
            .map_err(js_err)?;
        let writable: FileSystemWritableFileStream = writable.unchecked_into();
        let write = writable.write_with_u8_array(sealed).map_err(js_err)?;
        JsFuture::from(write).await.map_err(js_err)?;
        // The commit. The entry's bytes go from none to all of them here.
        JsFuture::from(writable.close()).await.map_err(js_err)?;

        Ok(())
    }

    async fn read(&self, world_id: Uuid, fingerprint: &Fingerprint) -> Result<Option<Vec<u8>>> {
        let Some(blob) = self.snapshot(world_id, fingerprint).await? else {
            return Ok(None);
        };
        // FR-021. An incomplete file is somebody's write in flight, or the
        // remains of one that died; either way it is not content, and — the
        // part that matters — it is not ours to delete. Callers treat this
        // exactly like a miss and fetch, which also repairs it: the next
        // write of this content targets this same name.
        if !BlobShape::of(Some(blob.size() as usize)).is_readable() {
            return Ok(None);
        }
        let buffer = JsFuture::from(blob.array_buffer()).await.map_err(js_err)?;
        Ok(Some(Uint8Array::new(&buffer).to_vec()))
    }

    async fn shape(&self, world_id: Uuid, fingerprint: &Fingerprint) -> Result<BlobShape> {
        Ok(BlobShape::of(
            self.snapshot(world_id, fingerprint)
                .await?
                .map(|blob| blob.size() as usize),
        ))
    }

    async fn remove(&self, world_id: Uuid, fingerprint: &Fingerprint) -> Result<()> {
        let Some(world) = get_dir(&self.scope_root, &world_dir_name(world_id), false).await? else {
            return Ok(());
        };
        remove_entry(&world, &blob_file_name(fingerprint), false).await
    }

    async fn list(&self, world_id: Uuid) -> Result<Vec<Fingerprint>> {
        let Some(world) = get_dir(&self.scope_root, &world_dir_name(world_id), false).await? else {
            return Ok(Vec::new());
        };
        let names = list_names(&world).await?;
        Ok(names
            .iter()
            .filter_map(|name| fingerprint_from_file_name(name).ok())
            .collect())
    }

    async fn remove_world(&self, world_id: Uuid) -> Result<()> {
        remove_entry(&self.scope_root, &world_dir_name(world_id), true).await
    }

    async fn remove_scope(&self) -> Result<()> {
        let root = JsFuture::from(
            global_property("navigator")
                .and_then(|nav| {
                    js_sys::Reflect::get(&nav, &JsValue::from_str("storage")).map_err(js_err)
                })?
                .unchecked_into::<StorageManager>()
                .get_directory(),
        )
        .await
        .map_err(js_err)?;
        let root: FileSystemDirectoryHandle = root.unchecked_into();
        remove_entry(&root, self.scope.as_str(), true).await
    }
}

/// `getDirectoryHandle`, mapping "not found" to `None` rather than an error.
/// OPFS reports absence as a `NotFoundError` rejection, and a cache miss is
/// not an error condition here.
async fn get_dir(
    parent: &FileSystemDirectoryHandle,
    name: &str,
    create: bool,
) -> Result<Option<FileSystemDirectoryHandle>> {
    let opts = FileSystemGetDirectoryOptions::new();
    opts.set_create(create);
    match JsFuture::from(parent.get_directory_handle_with_options(name, &opts)).await {
        Ok(handle) => Ok(Some(handle.unchecked_into())),
        Err(err) if !create && is_not_found(&err) => Ok(None),
        Err(err) => Err(js_err(err)),
    }
}

async fn get_file(
    parent: &FileSystemDirectoryHandle,
    name: &str,
    create: bool,
) -> Result<Option<FileSystemFileHandle>> {
    let opts = FileSystemGetFileOptions::new();
    opts.set_create(create);
    match JsFuture::from(parent.get_file_handle_with_options(name, &opts)).await {
        Ok(handle) => Ok(Some(handle.unchecked_into())),
        Err(err) if !create && is_not_found(&err) => Ok(None),
        Err(err) => Err(js_err(err)),
    }
}

async fn remove_entry(
    parent: &FileSystemDirectoryHandle,
    name: &str,
    recursive: bool,
) -> Result<()> {
    let opts = FileSystemRemoveOptions::new();
    opts.set_recursive(recursive);
    match JsFuture::from(parent.remove_entry_with_options(name, &opts)).await {
        Ok(_) => Ok(()),
        Err(err) if is_not_found(&err) => Ok(()),
        Err(err) => Err(js_err(err)),
    }
}

/// Directory listing via the async iterator OPFS exposes. Only names are
/// needed, so `keys()` is used rather than `entries()`.
async fn list_names(dir: &FileSystemDirectoryHandle) -> Result<Vec<String>> {
    let iter = dir.keys();
    let mut names = Vec::new();
    loop {
        let next = iter.next().map_err(js_err)?;
        let step = JsFuture::from(next).await.map_err(js_err)?;
        let done = js_sys::Reflect::get(&step, &JsValue::from_str("done"))
            .map_err(js_err)?
            .as_bool()
            .unwrap_or(true);
        if done {
            break;
        }
        let value = js_sys::Reflect::get(&step, &JsValue::from_str("value")).map_err(js_err)?;
        if let Some(name) = value.as_string() {
            names.push(name);
        }
    }
    Ok(names)
}

/// Distinguish "this entry does not exist" from a real platform failure.
/// Checked by `name` because `DOMException` is not reliably an `Error`
/// subclass across engines.
fn is_not_found(err: &JsValue) -> bool {
    js_sys::Reflect::get(err, &JsValue::from_str("name"))
        .ok()
        .and_then(|name| name.as_string())
        .is_some_and(|name| name == "NotFoundError")
}
