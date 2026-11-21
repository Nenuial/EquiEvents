-- equievents.ch: accounts, shows and their operation logs (docs/architecture/server.md).

-- Personal accounts (show offices register; for now made by an administrator).
CREATE TABLE users (
    id            bigserial PRIMARY KEY,
    name          text NOT NULL,
    email         text NOT NULL UNIQUE,
    password_hash text NOT NULL,
    admin         boolean NOT NULL DEFAULT false,
    created_at    timestamptz NOT NULL DEFAULT now()
);

-- A signed-in app (desktop, phone): its token, kept only as a SHA-256 hash.
CREATE TABLE sessions (
    token_hash text PRIMARY KEY,
    user_id    bigint NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    device     text NOT NULL DEFAULT '',
    created_at timestamptz NOT NULL DEFAULT now(),
    last_used  timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL
);
CREATE INDEX sessions_user ON sessions (user_id);

-- A show: the master copy of its operation log is here.
CREATE TABLE shows (
    id         text PRIMARY KEY,
    name       text NOT NULL,
    -- Swiss Equestrian's event id, when the show has one.
    event_id   bigint,
    created_by bigint REFERENCES users (id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

-- Who works on a show: its owner, its office.
CREATE TABLE show_members (
    show_id text NOT NULL REFERENCES shows (id) ON DELETE CASCADE,
    user_id bigint NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    role    text NOT NULL DEFAULT 'office' CHECK (role IN ('owner', 'office')),
    PRIMARY KEY (show_id, user_id)
);
CREATE INDEX show_members_user ON show_members (user_id);

-- A show's operations, each stored once (by its id), in the order they arrived (`seq`: an app
-- pulls what came after the last one it has). The state is computed from them, never stored.
CREATE TABLE operations (
    seq         bigserial PRIMARY KEY,
    show_id     text NOT NULL REFERENCES shows (id) ON DELETE CASCADE,
    op_id       text NOT NULL,
    op          jsonb NOT NULL,
    pushed_by   bigint REFERENCES users (id) ON DELETE SET NULL,
    received_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (show_id, op_id)
);
CREATE INDEX operations_show_seq ON operations (show_id, seq);
