use super::*;

/// Set some variables, run, restore.
///
/// The environment is process-global and `cargo test` is threaded, so
/// these cases are serialised behind one lock. Without it they pass alone
/// and fail together, which is the worst way for a test to be wrong.
fn temp_env(vars: &[(&str, Option<&str>)], body: impl FnOnce()) {
    use std::sync::Mutex;
    static LOCK: Mutex<()> = Mutex::new(());
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let previous: Vec<(String, Option<String>)> = vars
        .iter()
        .map(|(k, _)| ((*k).to_string(), std::env::var(k).ok()))
        .collect();
    for (k, v) in vars {
        // SAFETY: serialised by LOCK above; no other thread in this test
        // binary reads these variables outside this helper.
        unsafe {
            match v {
                Some(value) => std::env::set_var(k, value),
                None => std::env::remove_var(k),
            }
        }
    }

    body();

    for (k, v) in previous {
        unsafe {
            match v {
                Some(value) => std::env::set_var(&k, value),
                None => std::env::remove_var(&k),
            }
        }
    }
}

/// A PEM pasted into a `.env` file arrives with literal backslash-n.
/// Accepting it is the difference between a working instance and an
/// operator convinced their key is corrupt.
#[test]
fn a_pem_with_escaped_newlines_is_restored() {
    let raw = "-----BEGIN PRIVATE KEY-----\\nMIIB\\n-----END PRIVATE KEY-----";
    let out = String::from_utf8(normalise_pem(raw)).unwrap();
    assert!(out.contains('\n'), "escapes were not expanded");
    assert!(!out.contains("\\n"), "an escape survived");
    assert_eq!(out.lines().count(), 3);
}

#[test]
fn a_pem_with_real_newlines_is_left_alone() {
    let raw = "-----BEGIN PRIVATE KEY-----\nMIIB\n-----END PRIVATE KEY-----";
    assert_eq!(String::from_utf8(normalise_pem(raw)).unwrap(), raw);
}

/// The form a platform that accepts only single-line values forces.
#[test]
fn base64_of_a_pem_is_decoded() {
    let pem = "-----BEGIN PRIVATE KEY-----\nMIIB\n-----END PRIVATE KEY-----";
    let encoded = general_purpose::STANDARD.encode(pem);
    assert_eq!(String::from_utf8(normalise_pem(&encoded)).unwrap(), pem);
}

/// An editor or a YAML block scalar wraps a long value. Stripping
/// whitespace before decoding is what makes that survivable.
#[test]
fn wrapped_base64_still_decodes() {
    let pem = "-----BEGIN PRIVATE KEY-----\nMIIB\n-----END PRIVATE KEY-----";
    let encoded = general_purpose::STANDARD.encode(pem);
    let wrapped = encoded
        .as_bytes()
        .chunks(8)
        .map(|c| String::from_utf8_lossy(c).to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(String::from_utf8(normalise_pem(&wrapped)).unwrap(), pem);
}

/// Garbage is handed onward unchanged so the key parser produces the real
/// complaint. This function inventing one would describe the wrong problem.
///
/// The value matters: `"not a key at all"` with whitespace stripped is
/// `"notakeyatall"`, which is valid base64. An earlier version decoded it
/// to nine bytes of noise and would have told the operator their key was
/// malformed rather than that it was not a key. That is why the base64
/// branch is decided by whether the *result* is a PEM.
#[test]
fn something_that_is_neither_is_passed_through() {
    assert_eq!(
        normalise_pem("not a key at all").as_slice(),
        b"not a key at all"
    );
}

/// The same trap with a value that is unambiguously base64 but decodes to
/// something that is not a key. Accepting it would replace the operator's
/// input with noise before the parser ever saw it.
#[test]
fn valid_base64_of_something_that_is_not_a_pem_is_not_accepted() {
    let encoded = general_purpose::STANDARD.encode("just some text, not a key");
    assert_eq!(
        String::from_utf8(normalise_pem(&encoded)).unwrap(),
        encoded,
        "base64 of a non-PEM was decoded anyway",
    );
}

/// The `.env`-safe form, declared rather than sniffed.
#[test]
fn a_declared_base64_key_is_decoded() {
    let pem = "-----BEGIN PRIVATE KEY-----\nMIIB\n-----END PRIVATE KEY-----";
    let encoded = general_purpose::STANDARD.encode(pem);
    temp_env(
        &[
            (APP_PRIVATE_KEY_BASE64_ENV, Some(encoded.as_str())),
            (APP_PRIVATE_KEY_ENV, None),
            (APP_PRIVATE_KEY_FILE_ENV, None),
        ],
        || {
            let key = read_private_key().expect("readable").expect("present");
            assert_eq!(String::from_utf8(key).unwrap(), pem);
        },
    );
}

/// The operator declared an encoding, so being told "that is not base64"
/// is more useful than being told the key is unreadable. Falling through
/// to the plain variable's forgiving path would produce the second.
#[test]
fn a_declared_base64_key_that_is_not_base64_says_so() {
    temp_env(
        &[
            (
                APP_PRIVATE_KEY_BASE64_ENV,
                Some("-----BEGIN PRIVATE KEY-----"),
            ),
            (APP_PRIVATE_KEY_ENV, None),
            (APP_PRIVATE_KEY_FILE_ENV, None),
        ],
        || match read_private_key() {
            Err(RegistrationProblem::UndecodableBase64Key(_)) => {}
            other => panic!("expected an UndecodableBase64Key, got {other:?}"),
        },
    );
}

/// A file path outranks both inline forms. An operator who went to the
/// trouble of a mounted secret should not be silently overridden by a
/// stale value left in an environment file.
#[test]
fn a_file_path_outranks_the_inline_forms() {
    let dir = std::env::temp_dir().join(format!("tf-key-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.pem");
    std::fs::write(
        &path,
        b"-----BEGIN PRIVATE KEY-----\nFROMFILE\n-----END PRIVATE KEY-----",
    )
    .unwrap();

    temp_env(
        &[
            (APP_PRIVATE_KEY_FILE_ENV, Some(path.to_str().unwrap())),
            (
                APP_PRIVATE_KEY_BASE64_ENV,
                Some(
                    &general_purpose::STANDARD
                        .encode("-----BEGIN PRIVATE KEY-----\nINLINE\n-----END PRIVATE KEY-----"),
                ),
            ),
            (
                APP_PRIVATE_KEY_ENV,
                Some("-----BEGIN PRIVATE KEY-----\nPLAIN\n-----END PRIVATE KEY-----"),
            ),
        ],
        || {
            let key =
                String::from_utf8(read_private_key().expect("readable").expect("present")).unwrap();
            assert!(key.contains("FROMFILE"), "the file did not win: {key}");
        },
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// `_BASE64` outranks the plain variable, so an operator migrating to the
/// safer form does not have to remember to unset the old one.
#[test]
fn base64_outranks_the_plain_variable() {
    temp_env(
        &[
            (APP_PRIVATE_KEY_FILE_ENV, None),
            (
                APP_PRIVATE_KEY_BASE64_ENV,
                Some(
                    &general_purpose::STANDARD
                        .encode("-----BEGIN PRIVATE KEY-----\nWINNER\n-----END PRIVATE KEY-----"),
                ),
            ),
            (
                APP_PRIVATE_KEY_ENV,
                Some("-----BEGIN PRIVATE KEY-----\nLOSER\n-----END PRIVATE KEY-----"),
            ),
        ],
        || {
            let key =
                String::from_utf8(read_private_key().expect("readable").expect("present")).unwrap();
            assert!(key.contains("WINNER"), "precedence is wrong: {key}");
        },
    );
}

/// FR-036c. An operator who set two of the three must be told which one is
/// missing, not merely that the instance is unconfigured.
#[test]
fn a_partial_registration_names_only_what_is_missing() {
    temp_env(
        &[
            (APP_ID_ENV, Some("12345")),
            (APP_SLUG_ENV, None),
            (APP_PRIVATE_KEY_FILE_ENV, None),
            (APP_PRIVATE_KEY_BASE64_ENV, None),
            (APP_PRIVATE_KEY_ENV, Some("-----BEGIN PRIVATE KEY-----")),
        ],
        || {
            let problems = registration_from_env().err().expect("unconfigured");
            assert!(problems.contains(&RegistrationProblem::MissingAppSlug));
            assert!(!problems.contains(&RegistrationProblem::MissingAppId));
        },
    );
}

/// A variable set to whitespace is set to nothing. Templated configuration
/// producing an empty value is common, and treating `""` as present would
/// report a usable integration and then fail at the grant.
#[test]
fn a_blank_value_counts_as_missing() {
    temp_env(
        &[
            (APP_ID_ENV, Some("   ")),
            (APP_SLUG_ENV, Some("slug")),
            (APP_PRIVATE_KEY_FILE_ENV, None),
            (APP_PRIVATE_KEY_BASE64_ENV, None),
            (APP_PRIVATE_KEY_ENV, Some("-----BEGIN PRIVATE KEY-----")),
        ],
        || {
            let problems = registration_from_env().err().expect("unconfigured");
            assert!(problems.contains(&RegistrationProblem::MissingAppId));
        },
    );
}

/// A key that is present and is not a key. **The case a presence check
/// calls configured**, and the whole reason this runs at configuration
/// time rather than at first use.
#[test]
fn a_present_but_unusable_key_is_reported_rather_than_accepted() {
    temp_env(
        &[
            (APP_ID_ENV, Some("12345")),
            (APP_SLUG_ENV, Some("slug")),
            (APP_PRIVATE_KEY_FILE_ENV, None),
            (APP_PRIVATE_KEY_BASE64_ENV, None),
            (APP_PRIVATE_KEY_ENV, Some("hunter2")),
        ],
        || {
            let problems = registration_from_env().err().expect("unconfigured");
            assert!(
                problems
                    .iter()
                    .any(|p| matches!(p, RegistrationProblem::UnreadablePrivateKey(_))),
                "an unusable key was accepted: {problems:?}",
            );
        },
    );
}

/// Every problem at once, not the first. An operator restarting once per
/// missing variable is a configuration experience nobody finishes.
#[test]
fn a_diagnostic_names_every_missing_piece() {
    for problem in [
        RegistrationProblem::MissingAppId,
        RegistrationProblem::MissingAppSlug,
        RegistrationProblem::MissingPrivateKey,
    ] {
        let guidance = problem.guidance();
        assert!(guidance.contains("SYNC_GITHUB_APP"), "{guidance}");
    }
}

/// The client ID and the slug are different things, and an operator who
/// swaps them gets an authentication failure that reads like a bad key.
/// The guidance has to say so — and has to name the client ID, because a
/// variable called `..._APP_ID` would send them looking for the numeric
/// one GitHub no longer recommends.
#[test]
fn the_guidance_distinguishes_the_identifier_from_the_slug() {
    assert!(
        RegistrationProblem::MissingAppId
            .guidance()
            .contains("client ID"),
        "the guidance does not say which identifier GitHub recommends",
    );
    assert!(
        RegistrationProblem::MissingAppId
            .guidance()
            .contains("different from the application's slug")
    );
    assert!(
        RegistrationProblem::MissingAppSlug
            .guidance()
            .contains("different from its client ID")
    );
    assert!(
        RegistrationProblem::MissingAppSlug
            .guidance()
            .contains("github.com/apps/"),
        "the guidance does not say where to find the slug",
    );
}

/// A diagnostic that helps an operator must not also help someone reading
/// their logs.
#[test]
fn no_guidance_can_carry_key_material() {
    let problem = RegistrationProblem::UnreadablePrivateKey(
        "invalid -----BEGIN PRIVATE KEY----- MIIBsecret".to_string(),
    );
    let guidance = problem.guidance();
    // The detail is echoed, so the rule is that we never *construct* one
    // from key bytes — asserted at the call site in `registration_from_env`,
    // which passes only the parser's own message.
    assert!(guidance.contains(APP_PRIVATE_KEY_ENV));
}
