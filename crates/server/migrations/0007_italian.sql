-- Italian e-mails (the user, 2026-11-21: the site and its e-mails in the app's three languages).
ALTER TABLE users DROP CONSTRAINT users_language_check;
ALTER TABLE users ADD CONSTRAINT users_language_check CHECK (language IN ('fr', 'de', 'it'));
