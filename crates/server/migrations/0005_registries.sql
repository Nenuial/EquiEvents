-- Swiss Equestrian's riders and horses, per account (the user, 2026-11-21): downloaded by the
-- server with the key of one of the account's shows, shared by all its shows and computers.
-- As delivered (getPersonen, getPferde): the riders carry their contact data.
CREATE TABLE registries (
    user_id       bigint PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    persons       jsonb NOT NULL,
    horses        jsonb NOT NULL,
    persons_count integer NOT NULL,
    horses_count  integer NOT NULL,
    downloaded_at timestamptz NOT NULL DEFAULT now()
);
