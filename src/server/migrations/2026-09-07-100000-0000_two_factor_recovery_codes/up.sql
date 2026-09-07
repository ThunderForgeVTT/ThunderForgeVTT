-- Spec 041 US2 (FR-006 … FR-011): the way back in when the phone is gone.
--
-- A second factor with no recovery is a way to lose an account, and the person
-- it happens to has done nothing wrong. Ten codes are issued when an enrolment
-- is confirmed, shown once, and each works exactly once.
--
-- `code_hash` is an Argon2id PHC string, written by the same `hash_password`
-- that stores a password and the admin bootstrap code — the precedent chosen
-- in research R5. A recovery code is a single-use, human-transcribed,
-- account-granting secret, which is what the bootstrap code is; it is not a
-- share link, so it is not stored raw, and it never needs reading back, so it
-- is not encrypted either. FR-009 ("the instance cannot display them again")
-- is then true because there is nothing left to display, rather than because
-- an access rule says not to.
--
-- There is deliberately no `code_prefix` or other lookup hint. It would turn
-- verification into one hash instead of ten, and it would also make the codes
-- partially readable — which is the property being removed.
CREATE TABLE user_recovery_codes (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code_hash TEXT NOT NULL,
    -- The single-use guard, and not a flag anybody reads first: a code is spent
    -- by `UPDATE … SET used_at = now() WHERE id = $1 AND used_at IS NULL`, and
    -- zero rows updated is the refusal (FR-008). Two simultaneous
    -- presentations of one code cannot both win.
    used_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- The only query is "this account's unspent codes" — the one on the recovery
-- path, and the one behind "how many do I have left" (FR-011).
CREATE INDEX idx_user_recovery_codes_unspent
    ON user_recovery_codes (user_id)
    WHERE used_at IS NULL;
