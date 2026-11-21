-- Support (the user, 2026-11-21): accounts that help others by acting as them (impersonating)
-- in the online version. Administrators do all that and administer accounts (passwords,
-- deletion, roles).
ALTER TABLE users ADD COLUMN support boolean NOT NULL DEFAULT false;
-- A session acting as another account (administrators and support): in the online version it
-- works as that account; its own staff powers stay its own.
ALTER TABLE sessions ADD COLUMN acting_as bigint REFERENCES users (id) ON DELETE SET NULL;
