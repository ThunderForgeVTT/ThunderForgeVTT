//! Encrypted blob storage in OPFS, addressed by fingerprint (T024).
//!
//! Spec 028 FR-003/FR-010/FR-016/FR-018/FR-046, data-model.md "OPFS layout".
//!
//! ```text
//! /{user_scope}/{world_id}/{fingerprint}.bin      # encrypted bytes
//! ```
//!
//! # Why the filename is the fingerprint and not the item id
//!
//! Three properties fall out of that one choice, and none of them are
//! available if blobs are named after the thing that references them:
//!
//! 1. **Identical content is stored once.** A token used in six scenes, or a
//!    map shared between two worlds within a scope, is one file. Nothing has
//!    to notice the duplication; the path already collides.
//! 2. **A peer-supplied blob lands where its own hash says.** There is no
//!    step where a peer gets to tell us what to call the bytes it sent. We
//!    hash what arrived and that determines the path, so a hostile peer's
//!    only reachable outcome is writing content we would have accepted from
//!    the server anyway (FR-046, FR-047).
//! 3. **A blob is self-validating.** Bytes that do not hash to their own
//!    filename are corrupt *by definition* — no side table to consult, no
//!    second write that could have been lost in a crash. That is what makes
//!    the FR-018 self-check cheap and FR-019's repair decidable without
//!    re-downloading anything.
//!
//! The world id stays in the path even though the fingerprint alone would be
//! unique, because eviction works in whole worlds before it works in items
//! (data-model.md, `BudgetPlan`), and "delete this directory" is a far
//! cheaper operation than "enumerate and filter". The cost is that content
//! shared across two worlds is stored twice; that is the deliberate trade.

use std::fmt;

use thunderforge_cache_core::Fingerprint;
use uuid::Uuid;

/// Extension every encrypted blob carries.
pub const BLOB_EXTENSION: &str = ".bin";

/// The longest a `user_scope` segment may be.
const MAX_SCOPE_LEN: usize = 64;

/// Why a value was refused as a path segment.
///
/// Path construction is validating rather than sanitising on purpose. A
/// caller that hands us `..` has a bug, and quietly rewriting it to something
/// safe hides the bug while still writing a file somewhere unintended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    /// Empty, or longer than [`MAX_SCOPE_LEN`].
    ScopeLength { found: usize },
    /// Contained something outside `[A-Za-z0-9_-]`. That set excludes `/`,
    /// `\`, `.` and NUL, which is the entire traversal surface.
    ScopeCharacter { found: char },
    /// A filename that was not `<64 lowercase hex>.bin`.
    NotABlobName,
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScopeLength { found } => {
                write!(
                    f,
                    "user scope must be 1..={MAX_SCOPE_LEN} characters, found {found}"
                )
            }
            Self::ScopeCharacter { found } => {
                write!(f, "user scope may not contain {found:?}")
            }
            Self::NotABlobName => f.write_str("not a `<fingerprint>.bin` blob name"),
        }
    }
}

impl std::error::Error for PathError {}

/// The per-user root directory name.
///
/// Two users signed into the same browser profile must never share cached
/// bytes (FR-003), and the separation has to hold before any key is
/// involved — a shared directory would leak *which* content exists even if
/// the bytes are unreadable. So the scope is a path segment, not a field.
///
/// The value is opaque: callers derive it from the session, and this type's
/// only job is to guarantee whatever they derived is safe to write.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct UserScope(String);

impl UserScope {
    /// Accept a caller-derived scope, refusing anything unsafe as a segment.
    pub fn new(scope: impl Into<String>) -> Result<Self, PathError> {
        let scope = scope.into();
        if scope.is_empty() || scope.len() > MAX_SCOPE_LEN {
            return Err(PathError::ScopeLength { found: scope.len() });
        }
        if let Some(bad) = scope
            .chars()
            .find(|c| !(c.is_ascii_alphanumeric() || *c == '-' || *c == '_'))
        {
            return Err(PathError::ScopeCharacter { found: bad });
        }
        Ok(Self(scope))
    }

    /// Derive a scope for an authenticated user.
    ///
    /// Hashed rather than using the uuid directly so that the directory
    /// listing of a shared machine does not enumerate who has signed in on
    /// it. This is obfuscation, not secrecy — the encryption is what protects
    /// the content — but it costs nothing and the plaintext id buys nothing.
    pub fn for_user(user_id: Uuid) -> Self {
        let digest = Fingerprint::of_bytes(user_id.as_bytes()).to_hex();
        // Always valid: hex is a subset of the accepted alphabet.
        Self(digest[..32].to_string())
    }

    /// The directory name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UserScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Where one encrypted blob lives.
///
/// Pure and native-testable: constructing a path must never require a
/// browser, because getting it wrong is how a cache writes outside its own
/// directory.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BlobPath {
    scope: UserScope,
    world_id: Uuid,
    fingerprint: Fingerprint,
}

impl BlobPath {
    /// Derive the path content with this fingerprint occupies in this world.
    pub fn new(scope: UserScope, world_id: Uuid, fingerprint: Fingerprint) -> Self {
        Self {
            scope,
            world_id,
            fingerprint,
        }
    }

    /// The user-scope directory, directly under the OPFS root.
    pub fn scope_dir(&self) -> &str {
        self.scope.as_str()
    }

    /// The per-world directory, so eviction can drop a world wholesale.
    pub fn world_dir(&self) -> String {
        world_dir_name(self.world_id)
    }

    /// The blob's filename — the fingerprint it must hash to.
    pub fn file_name(&self) -> String {
        blob_file_name(&self.fingerprint)
    }

    /// The fingerprint this path asserts its contents hash to.
    pub fn fingerprint(&self) -> &Fingerprint {
        &self.fingerprint
    }

    /// The full path, for diagnostics and tests. OPFS itself is navigated
    /// handle by handle, so nothing in the write path parses this string.
    pub fn to_path_string(&self) -> String {
        format!(
            "/{}/{}/{}",
            self.scope_dir(),
            self.world_dir(),
            self.file_name()
        )
    }
}

impl fmt::Display for BlobPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_path_string())
    }
}

/// The directory name for a world. Hyphenated lowercase uuid — `Uuid`'s
/// `Display` already excludes every character a path could object to.
pub fn world_dir_name(world_id: Uuid) -> String {
    world_id.to_string()
}

/// The filename content with this fingerprint must be stored under.
pub fn blob_file_name(fingerprint: &Fingerprint) -> String {
    format!("{}{BLOB_EXTENSION}", fingerprint.to_hex())
}

/// Recover the fingerprint a stored file claims to hold.
///
/// This is the inverse the FR-019 repair pass needs: walking the directory
/// tells you what the cache actually holds, without opening a single file and
/// without trusting the index. A name that does not parse is not ours and is
/// left alone rather than deleted — a foreign file in our directory is a
/// situation we did not create and should not compound.
pub fn fingerprint_from_file_name(name: &str) -> Result<Fingerprint, PathError> {
    let hex = name
        .strip_suffix(BLOB_EXTENSION)
        .ok_or(PathError::NotABlobName)?;
    Fingerprint::from_hex(hex).map_err(|_| PathError::NotABlobName)
}

#[cfg(target_arch = "wasm32")]
pub use wasm::OpfsStore;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use js_sys::Uint8Array;
    use thunderforge_cache_core::{Fingerprint, fingerprint};
    use uuid::Uuid;
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{
        FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemGetDirectoryOptions,
        FileSystemGetFileOptions, FileSystemRemoveOptions, FileSystemWritableFileStream,
        StorageManager,
    };

    use super::{BlobPath, UserScope, blob_file_name, world_dir_name};
    use crate::crypto::SessionKey;
    use crate::{CacheError, Result, global_property, js_err};

    /// The encrypted blob store rooted at this user's OPFS scope.
    pub struct OpfsStore {
        scope_root: FileSystemDirectoryHandle,
        scope: UserScope,
    }

    impl OpfsStore {
        /// Open (creating if absent) the scope directory for this user.
        pub async fn open(scope: UserScope) -> Result<Self> {
            let navigator = global_property("navigator")?;
            let storage =
                js_sys::Reflect::get(&navigator, &JsValue::from_str("storage")).map_err(js_err)?;
            if storage.is_undefined() || storage.is_null() {
                return Err(CacheError::Unsupported("OPFS (navigator.storage)"));
            }
            let storage: StorageManager = storage.unchecked_into();
            let root = JsFuture::from(storage.get_directory())
                .await
                .map_err(|_| CacheError::Unsupported("OPFS (getDirectory)"))?;
            let root: FileSystemDirectoryHandle = root.unchecked_into();
            let scope_root = get_dir(&root, scope.as_str(), true)
                .await?
                .ok_or(CacheError::Unsupported("OPFS directory creation"))?;
            Ok(Self { scope_root, scope })
        }

        /// The scope this store is confined to.
        pub fn scope(&self) -> &UserScope {
            &self.scope
        }

        /// Store bytes, encrypted, at the path their fingerprint dictates.
        ///
        /// The `expected` fingerprint is what the *server* promised for this
        /// content. Verification happens before anything is encrypted or
        /// written, so bytes that are not what they claim never reach disk —
        /// and because the verified fingerprint is also the filename, a
        /// caller cannot file good bytes under the wrong name even by
        /// mistake (FR-010, FR-046).
        pub async fn write_blob(
            &self,
            world_id: Uuid,
            expected: &Fingerprint,
            plaintext: &[u8],
            key: &SessionKey,
        ) -> Result<BlobPath> {
            fingerprint::verify(plaintext, expected)?;
            let path = BlobPath::new(self.scope.clone(), world_id, *expected);

            let sealed = key.seal(plaintext).await?;
            let world = get_dir(&self.scope_root, &path.world_dir(), true)
                .await?
                .ok_or(CacheError::Unsupported("OPFS directory creation"))?;
            let file = get_file(&world, &path.file_name(), true)
                .await?
                .ok_or(CacheError::Unsupported("OPFS file creation"))?;

            let writable = JsFuture::from(file.create_writable())
                .await
                .map_err(js_err)?;
            let writable: FileSystemWritableFileStream = writable.unchecked_into();
            let write = writable.write_with_u8_array(&sealed).map_err(js_err)?;
            JsFuture::from(write).await.map_err(js_err)?;
            JsFuture::from(writable.close()).await.map_err(js_err)?;

            Ok(path)
        }

        /// Read and decrypt the blob for a fingerprint.
        ///
        /// Returns `Ok(None)` for every recoverable condition — absent file,
        /// key we no longer hold, ciphertext that will not open, plaintext
        /// that does not hash to its own filename. All four mean the same
        /// thing to a caller (fetch it again), and collapsing them is what
        /// makes key loss indistinguishable from a cold cache (FR-016c).
        ///
        /// Content that fails to verify is deleted before returning. Leaving
        /// it would mean re-reading and re-failing on it forever, and it can
        /// never become readable again: the filename is a claim about the
        /// plaintext, so plaintext that disagrees is not a different version
        /// of anything, it is garbage occupying budget.
        pub async fn read_blob(
            &self,
            world_id: Uuid,
            expected: &Fingerprint,
            key: &SessionKey,
        ) -> Result<Option<Vec<u8>>> {
            let path = BlobPath::new(self.scope.clone(), world_id, *expected);
            let Some(world) = get_dir(&self.scope_root, &path.world_dir(), false).await? else {
                return Ok(None);
            };
            let Some(file) = get_file(&world, &path.file_name(), false).await? else {
                return Ok(None);
            };

            let sealed = read_all(&file).await?;
            let Some(plaintext) = key.open(&sealed).await? else {
                self.discard(&world, &path).await;
                return Ok(None);
            };
            if fingerprint::verify(&plaintext, expected).is_err() {
                self.discard(&world, &path).await;
                return Ok(None);
            }
            Ok(Some(plaintext))
        }

        /// Whether a blob file exists, without decrypting it.
        ///
        /// Presence is not proof of readability — only [`Self::read_blob`]
        /// can establish that — so this is for repair and accounting, never
        /// for deciding a fetch can be skipped.
        pub async fn has_blob(&self, world_id: Uuid, fingerprint: &Fingerprint) -> Result<bool> {
            let Some(world) = get_dir(&self.scope_root, &world_dir_name(world_id), false).await?
            else {
                return Ok(false);
            };
            Ok(get_file(&world, &blob_file_name(fingerprint), false)
                .await?
                .is_some())
        }

        /// Delete one blob. Absent is success — the postcondition is "not
        /// there", and it already holds.
        pub async fn remove_blob(&self, world_id: Uuid, fingerprint: &Fingerprint) -> Result<()> {
            let Some(world) = get_dir(&self.scope_root, &world_dir_name(world_id), false).await?
            else {
                return Ok(());
            };
            remove_entry(&world, &blob_file_name(fingerprint), false).await
        }

        /// Every fingerprint physically present for a world.
        ///
        /// The ground truth the FR-019 repair pass diffs the index against.
        /// Files whose names are not ours are skipped rather than reported.
        pub async fn list_fingerprints(&self, world_id: Uuid) -> Result<Vec<Fingerprint>> {
            let Some(world) = get_dir(&self.scope_root, &world_dir_name(world_id), false).await?
            else {
                return Ok(Vec::new());
            };
            let names = list_names(&world).await?;
            Ok(names
                .iter()
                .filter_map(|name| super::fingerprint_from_file_name(name).ok())
                .collect())
        }

        /// Drop a world's bytes wholesale — the coarse eviction step
        /// (data-model.md, `BudgetPlan`: whole worlds before individual
        /// items).
        pub async fn remove_world(&self, world_id: Uuid) -> Result<()> {
            remove_entry(&self.scope_root, &world_dir_name(world_id), true).await
        }

        /// Drop this user's entire cache directory.
        ///
        /// Sign-out (FR-016a) discards the *key* first and synchronously;
        /// this is the FR-016b reclamation that follows and may be slow. The
        /// ordering matters: a large store cannot be wiped instantly, and
        /// deletion alone would leave a readable window. Encryption is what
        /// closes that window, so the key must go first and this must not be
        /// treated as the thing that makes the cache unreadable.
        pub async fn remove_scope(&self) -> Result<()> {
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

        /// Best-effort removal of content that failed verification. The read
        /// already decided the answer is `None`; whether the delete lands is
        /// a matter of reclaiming space, not of correctness.
        async fn discard(&self, world: &FileSystemDirectoryHandle, path: &BlobPath) {
            let _ = remove_entry(world, &path.file_name(), false).await;
        }
    }

    /// `getDirectoryHandle`, mapping "not found" to `None` rather than an
    /// error. OPFS reports absence as a `NotFoundError` rejection, and a
    /// cache miss is not an error condition here.
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

    async fn read_all(file: &FileSystemFileHandle) -> Result<Vec<u8>> {
        let blob = JsFuture::from(file.get_file()).await.map_err(js_err)?;
        let blob: web_sys::File = blob.unchecked_into();
        let buffer = JsFuture::from(blob.array_buffer()).await.map_err(js_err)?;
        Ok(Uint8Array::new(&buffer).to_vec())
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp(byte: u8) -> Fingerprint {
        Fingerprint::of_bytes(&[byte])
    }

    #[test]
    fn path_follows_the_documented_layout() {
        let scope = UserScope::new("abc123").expect("valid scope");
        let world = Uuid::nil();
        let fingerprint = fp(1);
        let path = BlobPath::new(scope, world, fingerprint);
        assert_eq!(
            path.to_path_string(),
            format!(
                "/abc123/00000000-0000-0000-0000-000000000000/{}.bin",
                fingerprint.to_hex()
            )
        );
    }

    #[test]
    fn identical_content_shares_one_path() {
        // The dedup property, stated as a test: two different items whose
        // bytes are equal cannot occupy two files.
        let scope = UserScope::new("s").expect("valid scope");
        let world = Uuid::from_u128(7);
        let a = BlobPath::new(scope.clone(), world, Fingerprint::of_bytes(b"same"));
        let b = BlobPath::new(scope, world, Fingerprint::of_bytes(b"same"));
        assert_eq!(a.to_path_string(), b.to_path_string());
    }

    #[test]
    fn differing_content_never_shares_a_path() {
        let scope = UserScope::new("s").expect("valid scope");
        let world = Uuid::from_u128(7);
        let a = BlobPath::new(scope.clone(), world, Fingerprint::of_bytes(b"one"));
        let b = BlobPath::new(scope, world, Fingerprint::of_bytes(b"two"));
        assert_ne!(a.to_path_string(), b.to_path_string());
    }

    #[test]
    fn worlds_are_separate_directories_so_eviction_can_be_coarse() {
        let scope = UserScope::new("s").expect("valid scope");
        let fingerprint = fp(9);
        let a = BlobPath::new(scope.clone(), Uuid::from_u128(1), fingerprint);
        let b = BlobPath::new(scope, Uuid::from_u128(2), fingerprint);
        assert_ne!(a.world_dir(), b.world_dir());
        assert_eq!(a.file_name(), b.file_name());
    }

    #[test]
    fn scopes_isolate_users() {
        let a = UserScope::for_user(Uuid::from_u128(1));
        let b = UserScope::for_user(Uuid::from_u128(2));
        assert_ne!(a, b);
        assert_eq!(a, UserScope::for_user(Uuid::from_u128(1)));
    }

    #[test]
    fn derived_scope_does_not_leak_the_user_id() {
        let user = Uuid::from_u128(0x1234_5678);
        let scope = UserScope::for_user(user);
        assert!(!scope.as_str().contains(&user.to_string()));
        assert!(UserScope::new(scope.as_str()).is_ok());
    }

    #[test]
    fn traversal_is_refused_not_sanitised() {
        for bad in ["..", "a/b", "a\\b", "a.b", "a b", ""] {
            assert!(
                UserScope::new(bad).is_err(),
                "expected {bad:?} to be refused"
            );
        }
    }

    #[test]
    fn overlong_scope_is_refused() {
        let long = "a".repeat(MAX_SCOPE_LEN + 1);
        assert_eq!(
            UserScope::new(long),
            Err(PathError::ScopeLength {
                found: MAX_SCOPE_LEN + 1
            })
        );
    }

    #[test]
    fn file_name_round_trips_to_its_fingerprint() {
        let fingerprint = fp(42);
        let name = blob_file_name(&fingerprint);
        assert_eq!(fingerprint_from_file_name(&name), Ok(fingerprint));
    }

    #[test]
    fn foreign_file_names_are_not_mistaken_for_blobs() {
        for bad in ["notes.txt", "deadbeef.bin", "", ".bin", "ABCD.bin"] {
            assert_eq!(
                fingerprint_from_file_name(bad),
                Err(PathError::NotABlobName),
                "expected {bad:?} to be rejected"
            );
        }
    }
}
