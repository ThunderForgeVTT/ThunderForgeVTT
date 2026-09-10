DELETE FROM account_notices WHERE kind IN ('account_restored', 'actor_rescued');
ALTER TABLE account_notices DROP CONSTRAINT account_notices_kind_check;
ALTER TABLE account_notices ADD CONSTRAINT account_notices_kind_check CHECK (kind IN (
    'strike_recorded',
    'publishing_suspended',
    'account_disabled',
    'appeal_resolved',
    'share_taken_down',
    'adopted_copy_disabled',
    'adopted_copy_restored'
));

DROP TABLE IF EXISTS account_terminations;
