-- Reversing this destroys every recorded agreement and the archive of the words
-- they were made against. That is not a rollback, it is the loss of the evidence
-- the feature exists to hold — so if you are running this against anything but
-- a development database, stop and take a dump first.
DROP TABLE IF EXISTS attestations;
DROP TABLE IF EXISTS terms_versions;
