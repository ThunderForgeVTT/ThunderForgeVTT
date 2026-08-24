-- Spec 014 (FR-014): an immutable, append-only audit log of every
-- resolved dice roll — never updated after insert, same convention as
-- spec 012's world_lore_revisions. A row is only ever inserted after
-- thunderforge_dice::resolve() succeeds server-side.
CREATE TABLE world_roll_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    world_id UUID NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
    triggered_by UUID NOT NULL REFERENCES users(id),
    formula TEXT NOT NULL,
    bindings JSONB,
    detail JSONB NOT NULL,
    result_kind TEXT NOT NULL CHECK (result_kind IN ('total', 'success_count')),
    result_value DOUBLE PRECISION NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX world_roll_records_world_id_idx ON world_roll_records(world_id);
CREATE INDEX world_roll_records_created_at_idx ON world_roll_records(created_at);
