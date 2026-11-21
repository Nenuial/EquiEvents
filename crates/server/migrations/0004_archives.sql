-- Archives (decided 2026-11-15): a show's owner archives it by hand, or the owner's account
-- archives its shows on their own some weeks after their last day. Archived is how the shows
-- are listed, nothing more: an archived show works as before.

-- A show's last day: the event's, or a later day a class was moved to (from its operations).
ALTER TABLE shows ADD COLUMN last_day date;
-- When it was archived; when its owner took it out of the archives (it is then not archived
-- on its own again).
ALTER TABLE shows ADD COLUMN archived_at timestamptz;
ALTER TABLE shows ADD COLUMN unarchived_at timestamptz;
-- Archive the shows one owns this many weeks after their last day (none: by hand only).
ALTER TABLE users ADD COLUMN archive_after_weeks integer CHECK (archive_after_weeks BETWEEN 1 AND 104);

UPDATE shows s SET last_day = d.last
FROM (
    SELECT o.show_id, max(left(x.day, 10)::date) AS last
    FROM operations o,
         LATERAL (VALUES (o.op -> 'change' -> 'event' ->> 'to'), (o.op -> 'change' -> 'class' ->> 'date')) AS x(day)
    WHERE o.op -> 'change' ->> 'kind' IN ('EventImported', 'ClassSet') AND x.day ~ '^[0-9]{4}-[0-9]{2}-[0-9]{2}'
    GROUP BY o.show_id
) d
WHERE d.show_id = s.id;
