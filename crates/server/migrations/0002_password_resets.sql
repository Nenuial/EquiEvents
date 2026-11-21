-- Accounts are made by the people themselves on the site (and the apps); a forgotten password
-- is reset with a link sent by e-mail: its token, kept only as a SHA-256 hash, valid an hour,
-- used once.
CREATE TABLE password_resets (
    token_hash text PRIMARY KEY,
    user_id    bigint NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL
);
CREATE INDEX password_resets_user ON password_resets (user_id);
