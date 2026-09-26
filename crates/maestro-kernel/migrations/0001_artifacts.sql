-- The artifacts the kernel's artifact store holds, one row each (plan D2): the
-- digest names the stored file, `pins` counts the records that refer to it, and
-- garbage collection removes only an artifact whose pins are zero.
CREATE TABLE artifacts (
  digest TEXT PRIMARY KEY NOT NULL,
  bytes INTEGER NOT NULL CHECK (bytes >= 0),
  media TEXT NOT NULL,
  pins INTEGER NOT NULL DEFAULT 0 CHECK (pins >= 0),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
