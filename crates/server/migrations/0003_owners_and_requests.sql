-- A show's key decides who owns it (decision 2026-11-14): the first account to enter a Swiss
-- Equestrian event's key owns the event's show; other accounts with the key ask the owner for
-- access, or for the ownership. One owner per show; the others are collaborators.

-- The language of a person's e-mails (the site's page they signed up on, or the app's).
ALTER TABLE users ADD COLUMN language text NOT NULL DEFAULT 'fr' CHECK (language IN ('fr', 'de'));

-- An event has one show, opened with its key (kept here: the server talks to Swiss Equestrian).
ALTER TABLE shows ADD COLUMN access_key text;
CREATE UNIQUE INDEX shows_event ON shows (event_id) WHERE event_id IS NOT NULL;

ALTER TABLE show_members DROP CONSTRAINT show_members_role_check;
UPDATE show_members SET role = 'collaborator' WHERE role = 'office';
ALTER TABLE show_members ALTER COLUMN role SET DEFAULT 'collaborator';
ALTER TABLE show_members ADD CONSTRAINT show_members_role_check CHECK (role IN ('owner', 'collaborator'));
CREATE UNIQUE INDEX show_one_owner ON show_members (show_id) WHERE role = 'owner';

-- Asking a show's owner for access or for the ownership; one open request per person and show.
CREATE TABLE show_requests (
    id         bigserial PRIMARY KEY,
    show_id    text NOT NULL REFERENCES shows (id) ON DELETE CASCADE,
    user_id    bigint NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind       text NOT NULL CHECK (kind IN ('access', 'ownership')),
    status     text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'accepted', 'refused', 'withdrawn')),
    created_at timestamptz NOT NULL DEFAULT now(),
    decided_at timestamptz,
    decided_by bigint REFERENCES users (id) ON DELETE SET NULL
);
CREATE UNIQUE INDEX show_requests_pending ON show_requests (show_id, user_id) WHERE status = 'pending';
CREATE INDEX show_requests_user ON show_requests (user_id);
