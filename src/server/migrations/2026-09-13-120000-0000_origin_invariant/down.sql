DROP TRIGGER IF EXISTS world_ability_shares_origin_trigger ON world_ability_shares;
DROP TRIGGER IF EXISTS world_item_shares_origin_trigger ON world_item_shares;
DROP TRIGGER IF EXISTS world_actor_shares_origin_trigger ON world_actor_shares;
DROP TRIGGER IF EXISTS world_collection_members_origin_trigger ON world_collection_members;
DROP FUNCTION IF EXISTS refuse_content_that_may_not_leave();
DROP FUNCTION IF EXISTS content_origin(TEXT, UUID);
