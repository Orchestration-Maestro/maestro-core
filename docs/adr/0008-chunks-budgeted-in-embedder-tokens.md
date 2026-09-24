# Chunks are budgeted in the selected embedder's tokens

Status: accepted, 2026-09-23.

Chunk budgets (target 500, hard maximum 700, special tokens and context
included) are counted with the tokenizer of the embedding model actually
selected, through the model router's `/tokenize` endpoint for that model's GGUF.
This removes machine paths and a process spawn per count, and guarantees the
embedder never truncates a chunk. The existing native counter is kept as the
parity reference: both must return identical ordered token IDs on the
qualification fixtures.

## Consequences

The chunk profile's identity includes the counter's contract ID, so selecting a
different embedder creates a new chunk set and a new generation; chunk sets for
different embedders may coexist.
