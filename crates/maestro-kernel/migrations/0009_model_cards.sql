-- Immutable model identities, candidate evaluations, and explicit role selections.

CREATE TABLE model_cards (
  id TEXT PRIMARY KEY NOT NULL,
  collection_id TEXT NOT NULL REFERENCES collections (id),
  role TEXT NOT NULL CHECK (role IN ('embedder', 'reranker', 'answerer')),
  digest TEXT NOT NULL CHECK (length(digest) = 64 AND digest NOT GLOB '*[^0-9a-f]*'),
  card_json TEXT NOT NULL CHECK (json_valid(card_json)
    AND json_extract(card_json, '$.schema') = 'maestro-model-card/2'
    AND json_extract(card_json, '$.identity.role') = role),
  recorded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE (collection_id, digest),
  UNIQUE (collection_id, id)
) STRICT;

CREATE INDEX model_cards_by_collection_role ON model_cards (collection_id, role);

CREATE TABLE model_evaluations (
  id TEXT PRIMARY KEY NOT NULL,
  run_id TEXT NOT NULL CHECK (length(trim(run_id)) > 0),
  collection_id TEXT NOT NULL REFERENCES collections (id),
  card_id TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('embedder', 'reranker', 'answerer')),
  mode TEXT NOT NULL CHECK (mode IN ('real', 'synthetic')),
  generation_id INTEGER,
  disposition TEXT NOT NULL CHECK (disposition IN ('eligible', 'ineligible', 'failed', 'blocked', 'interrupted')),
  manifest_digest TEXT NOT NULL CHECK (length(manifest_digest) = 64 AND manifest_digest NOT GLOB '*[^0-9a-f]*'),
  report_digest TEXT NOT NULL CHECK (length(report_digest) = 64 AND report_digest NOT GLOB '*[^0-9a-f]*'),
  recorded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (collection_id, card_id) REFERENCES model_cards (collection_id, id),
  UNIQUE (collection_id, id)
) STRICT;

CREATE INDEX model_evaluations_by_collection ON model_evaluations (collection_id);
CREATE INDEX model_evaluations_by_card ON model_evaluations (card_id);

CREATE TABLE model_selections (
  id TEXT PRIMARY KEY NOT NULL,
  collection_id TEXT NOT NULL REFERENCES collections (id),
  role TEXT NOT NULL CHECK (role IN ('embedder', 'reranker', 'answerer')),
  card_id TEXT NOT NULL,
  evaluation_id TEXT NOT NULL,
  selected_by TEXT NOT NULL CHECK (length(trim(selected_by)) > 0),
  reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),
  recorded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  FOREIGN KEY (collection_id, card_id) REFERENCES model_cards (collection_id, id),
  FOREIGN KEY (collection_id, evaluation_id) REFERENCES model_evaluations (collection_id, id)
) STRICT;

CREATE INDEX model_selections_by_role ON model_selections (collection_id, role);

-- Model records never change or disappear. Guard id, rowid, and every unique
-- key before SQLite can apply conflict-driven replacement.
CREATE TRIGGER model_cards_are_never_replaced BEFORE INSERT ON model_cards
WHEN EXISTS (SELECT 1 FROM model_cards WHERE id = NEW.id OR rowid = NEW.rowid
             OR (collection_id = NEW.collection_id AND digest = NEW.digest))
BEGIN SELECT RAISE(ABORT, 'a model card registration is never replaced'); END;
CREATE TRIGGER model_cards_are_never_updated BEFORE UPDATE ON model_cards
BEGIN SELECT RAISE(ABORT, 'a model card registration is immutable'); END;
CREATE TRIGGER model_cards_are_never_deleted BEFORE DELETE ON model_cards
BEGIN SELECT RAISE(ABORT, 'a model card registration is immutable'); END;

CREATE TRIGGER model_evaluations_match_registration BEFORE INSERT ON model_evaluations
WHEN NOT EXISTS (
  SELECT 1 FROM model_cards
  WHERE model_cards.id = NEW.card_id AND model_cards.collection_id = NEW.collection_id
    AND model_cards.role = NEW.role)
  OR (NEW.generation_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM generations
    WHERE generations.id = NEW.generation_id AND generations.collection_id = NEW.collection_id))
  OR EXISTS (SELECT 1 FROM model_evaluations WHERE id = NEW.id OR rowid = NEW.rowid)
BEGIN SELECT RAISE(ABORT, 'model evaluation does not match its collection, role, card, or generation'); END;
CREATE TRIGGER model_evaluations_are_never_updated BEFORE UPDATE ON model_evaluations
BEGIN SELECT RAISE(ABORT, 'a model evaluation is immutable'); END;
CREATE TRIGGER model_evaluations_are_never_deleted BEFORE DELETE ON model_evaluations
BEGIN SELECT RAISE(ABORT, 'a model evaluation is immutable'); END;

CREATE TRIGGER model_selections_require_eligible_real_evaluation BEFORE INSERT ON model_selections
WHEN NOT EXISTS (
  SELECT 1 FROM model_cards
  JOIN model_evaluations ON model_evaluations.collection_id = model_cards.collection_id
    AND model_evaluations.card_id = model_cards.id
  WHERE model_cards.id = NEW.card_id AND model_cards.collection_id = NEW.collection_id
    AND model_cards.role = NEW.role
    AND model_evaluations.id = NEW.evaluation_id
    AND model_evaluations.role = NEW.role
    AND model_evaluations.disposition = 'eligible'
    AND model_evaluations.mode = 'real')
  OR EXISTS (SELECT 1 FROM model_selections WHERE id = NEW.id OR rowid = NEW.rowid)
BEGIN SELECT RAISE(ABORT, 'selection requires an eligible real evaluation of the same v2 card and role'); END;
CREATE TRIGGER model_selections_are_never_updated BEFORE UPDATE ON model_selections
BEGIN SELECT RAISE(ABORT, 'a model selection is immutable'); END;
CREATE TRIGGER model_selections_are_never_deleted BEFORE DELETE ON model_selections
BEGIN SELECT RAISE(ABORT, 'a model selection is immutable'); END;
