ALTER TABLE users RENAME COLUMN password_hash TO password;
ALTER TABLE users DROP COLUMN email;