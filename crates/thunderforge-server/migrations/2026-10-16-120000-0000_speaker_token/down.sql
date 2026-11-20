ALTER TABLE world_roll_records
    DROP COLUMN speaker_label,
    DROP COLUMN speaker_token_id;

ALTER TABLE world_chat_messages
    DROP COLUMN speaker_label,
    DROP COLUMN speaker_token_id;
