ALTER TABLE tokens ADD CONSTRAINT valid_coordinates CHECK (x >= 0 AND y >= 0);
