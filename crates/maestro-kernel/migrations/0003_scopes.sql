-- Grants (plan D4): a principal's right on a scope and on every scope below
-- it. One row for each principal, scope and right, so a later right adds rows,
-- not columns; the one right is read. Nothing is granted implicitly: a
-- principal with no row sees nothing. Each grant and each revocation is
-- journaled with its actor in the write that makes it; the row keeps who
-- granted it and when, and a revocation deletes it.
--
-- There is no table of scopes: nothing lists them, and the collections and
-- sources a scope names are recorded in their own tables.
CREATE TABLE grants (
  principal TEXT NOT NULL,
  scope TEXT NOT NULL,
  right TEXT NOT NULL CHECK (right IN ('read')),
  granted_by TEXT NOT NULL,
  granted_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (principal, scope, right)
) STRICT;
