-- Whether Swiss Equestrian accepted the show's key (the user, 2026-11-21): a past event's key,
-- from its Rosson export, cannot be checked any more and is taken on trust ("if they have the
-- export, they have the key"); a key Swiss Equestrian accepts wins over it.
ALTER TABLE shows ADD COLUMN key_checked boolean NOT NULL DEFAULT true;
