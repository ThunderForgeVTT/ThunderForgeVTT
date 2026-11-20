-- Spec 103: a chat line or a roll said as a token.
--
-- `speaker_token_id` is the token the line floats over; `speaker_label` is
-- that token's real name when it was said, which the API withholds from
-- players who may not read it. Both NULL: said as the author, as before.
-- A deleted token leaves the label, so the log still reads.
ALTER TABLE world_chat_messages
    ADD COLUMN speaker_token_id UUID NULL REFERENCES tokens(token_id) ON DELETE SET NULL,
    ADD COLUMN speaker_label TEXT NULL;

ALTER TABLE world_roll_records
    ADD COLUMN speaker_token_id UUID NULL REFERENCES tokens(token_id) ON DELETE SET NULL,
    ADD COLUMN speaker_label TEXT NULL;
