-- The world-scoped `world_tokens` table has been retired since ADR-040 moved
-- every token onto the scene-scoped `tokens` table. Nothing in the client read
-- or wrote it any more, and the GraphQL routes left over for it checked who was
-- signed in but not whether they belonged to the world — so they are removed
-- with it. ADR-040 noted no production data depends on it; its rows were test
-- residue with no scene to move them to.
DROP TABLE IF EXISTS world_tokens;
