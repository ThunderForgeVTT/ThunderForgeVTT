-- Phase: Native canvas authoring - door semantics on walls (FR-017)
ALTER TABLE walls ADD COLUMN door_state TEXT NOT NULL DEFAULT 'none'
  CHECK (door_state IN ('none', 'open', 'closed'));
