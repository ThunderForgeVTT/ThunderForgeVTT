ALTER TABLE worlds
    ADD COLUMN description TEXT,
    ADD COLUMN game_system_id VARCHAR,
    ADD COLUMN interface_pack_id VARCHAR;

CREATE UNIQUE INDEX idx_worlds_owner_name_unique
    ON worlds (created_by, lower(name));

ALTER TABLE policies
    ADD COLUMN world_id UUID;

ALTER TABLE policies
    ADD CONSTRAINT policies_world_id_fkey
    FOREIGN KEY (world_id) REFERENCES worlds (id) ON DELETE CASCADE;

CREATE INDEX idx_policies_world_id ON policies (world_id);
