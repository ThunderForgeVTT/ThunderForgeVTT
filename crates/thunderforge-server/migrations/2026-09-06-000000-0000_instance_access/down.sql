ALTER TABLE oauth_authorization_sessions DROP COLUMN IF EXISTS invitation_code;
DROP TABLE IF EXISTS instance_access_events;
DROP TABLE IF EXISTS instance_invitation_redemptions;
DROP TABLE IF EXISTS instance_invitations;
DROP TABLE IF EXISTS instance_access_settings;
