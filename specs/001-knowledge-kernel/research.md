# Research: Knowledge kernel and hybrid RAG

The measured answers behind the research rows of [plan.md](plan.md#research).
A row whose answer was "to measure" points here once its task has measured it.

## R7 Server-side BM25

**Question:** can Qdrant's server-side BM25 hold the French and English
analyzer policy?

**Answer: no.** Maestro computes the sparse vectors itself, with the
`bm25-en-fr/1` analyzer (T040). Qdrant keeps what works: the sparse index, IDF
weighting (`modifier: idf`), IDF per language (`params.idf.corpus`) and
fusion, all of which accept vectors the client computes.

Measured by T004 on 2026-09-25 against Qdrant **1.19.1**
(`qdrant-x86_64-unknown-linux-gnu.tar.gz`, SHA-256
`eef986e769d4d3e806dd2d546e1b4ecdd416211e54d34b4ed764fac7c58e1085`, equal to
the digest GitHub publishes for the asset; Qdrant publishes no checksum file),
bound to `127.0.0.1` with telemetry off, on the public sample below. The
stored vectors were decoded back into tokens (a token's ID is
`abs(murmur3_32(token, seed 0))`), so every token here was read from Qdrant,
not guessed.

### What fails

1. **Accents against French inflection.** Qdrant folds accents *before* the
   French stemmer runs, and no option changes the order. Without folding,
   `planifier`, `planifié` and `planifiée` meet, but `resultat` misses
   `résultat`, `tache` misses `tâche` and `execution` misses `exécution`. With
   folding, the accents meet and the inflections part:

   | Word | `french` | `french` with folding |
   | --- | --- | --- |
   | planifier | planifi | planifi |
   | planifié, planifiés | planifi | planif |
   | planifiée, planifiées | planifi | planifie |
   | planifiee, without its accent | planifie | planifie |
   | échoué, échoués | échou | echou |
   | échouée, échouées | échou | echoue |
   | résultat, résultats | résultat | resultat |
   | tâche, tâches | tâch | tach |

   Folding first also turns accented stopwords into tokens (`à` → `a`, `été`
   → `ete`), and `planif` matches `planificateur`.
2. **Identifiers.** The default tokenizer splits on `-`, `_`, `.`, `/` and
   `:`, so a passage holding only the parts of an identifier ties with, or
   beats, the passage holding the identifier. For `max_retries`, the decoy en04
   ties en01 at 6.2539; for `job-id`, the decoy en06 ranks first. The
   `whitespace` tokenizer keeps identifiers whole but also the punctuation
   touching them (`err-4012:`, `(job-id)`), so no identifier query passes;
   `multilingual` drops tokens made only of digits (`ERR-4012` → `err`);
   `prefix` fails like the default. English analysis alone fails `max_retries`
   and `job-id`: the failure does not depend on French.
3. **The policy is neither stored nor checked.** Analyzer options travel with
   every request, and the collection stores only `modifier: idf`. A misspelled
   key (`{"languag": "french"}`) is ignored with HTTP 200 and gives English
   analysis; `"language": "French"` or `"français"` silently turns stemming
   and stopwords off, with one WARN line per server process as the only trace.

### Results

No configuration or combination passes all 23 checks; the best pass 18.

| Configuration | Recall (15) | Identifiers (5) | Topic (3) |
| --- | --- | --- | --- |
| Default (English) | 5 | 3 | 3 |
| French for every passage and query | 4 | 4 | 3 |
| The language of each passage and query | 5 | 3 | 3 |
| The same, with folding | 11 | 3 | 3 |
| The same, with folding and the `prefix` tokenizer | 12 | 3 | 3 |
| Folded and unfolded vectors, fused with RRF | 12 | 3 | 3 |
| Language-neutral: folding, `whitespace`, no stemmer, no stopwords | 0 | 0 | 3 |

Every configuration that passes 18 fails `planifiee`, `planifier`,
`max_retries` and `job-id`. A phrase filter on a full-text payload index
restores the five identifiers, but only once maestro knows which query terms
are identifiers, and it does nothing for French.

### What works server-side

- One collection, even one sparse vector, holds French passages analysed as
  French beside English ones analysed as English. The language is set per
  passage and per query; nothing is detected.
- A query analysed in both languages and fused with RRF ranks the right
  passage first without knowing the query's language.
- `params.idf.corpus`, new in 1.19, scopes IDF to one language's passages.
- Nothing is fetched at first use. A run in a network namespace with loopback
  only gave the same tokens and scores as the networked run, and a socket
  watch of the networked run saw no connection outside loopback.
- Vectors computed by the client (folded, lowercased, identifiers kept whole,
  BM25 term weights) pass all five identifier checks under `modifier: idf`.

### The sample and its checks

Written for the spike, public: 22 passages, 11 in each language. In each
identifier pair, one occurrence touches punctuation, as in real text; en03,
en04, en06 and en08 are decoys that hold the parts of an identifier but not
the identifier.

| ID | Passage |
| --- | --- |
| en01 | The scheduler marks the run as failed once it reaches max_retries. |
| en02 | Error ERR-4012: the request header is missing. |
| en03 | Error ERR-4013 is raised when the worker cannot reach the queue. |
| en04 | The max timeout is thirty seconds, and retries are logged by the worker. |
| en05 | Every request must carry a job-id header so the gateway can trace it. |
| en06 | Each job receives an id when it is created and keeps it until deletion. |
| en07 | AgentPort is the interface that connects a worker to the scheduler. |
| en08 | The agent listens on a port chosen at startup. |
| en09 | Planned tasks keep their results in the artifact store. |
| en10 | A task writes one result when it completes. |
| en11 | Retry policy: failed jobs are retried with exponential backoff. |
| fr01 | Le planificateur abandonne après max_retries tentatives et marque l'exécution comme échouée. |
| fr02 | L'erreur ERR-4012 signale qu'un en-tête manque dans la requête. |
| fr03 | Chaque requête doit porter l'en-tête (job-id) pour être tracée. |
| fr04 | Grâce à AgentPort, chaque exécutant rejoint le planificateur. |
| fr05 | Le résultat de la tâche est enregistré dans le magasin d'artefacts. |
| fr06 | Les résultats des tâches planifiées sont conservés trente jours. |
| fr07 | Une exécution planifiée démarre à l'heure prévue. |
| fr08 | Il faut planifier le lot avant de lancer l'exécution. |
| fr09 | Journal sans accents : resultat de la tache planifiee, statut termine. |
| fr10 | Politique de relance : les travaux échoués sont relancés avec un délai exponentiel. |
| fr11 | Le lot a été planifié hier soir par l'opératrice. |

| Check | Query | Language | Expected | Decoy |
| --- | --- | --- | --- | --- |
| Accents | `resultat` | fr | fr05, fr06, fr09 | |
| Accents | `résultat` | fr | fr05, fr06, fr09 | |
| Accents | `tache` | fr | fr05, fr06, fr09 | |
| Accents | `tâche` | fr | fr05, fr06, fr09 | |
| Accents | `planifiee` | fr | fr06, fr07, fr08, fr09, fr11 | |
| Accents | `execution` | fr | fr01, fr07, fr08 | |
| Plurals | `task` | en | en09, en10 | |
| Plurals | `results` | en | en09, en10 | |
| Plurals | `retry` | en | en04, en11 | |
| Plurals | `résultats` | fr | fr05, fr06, fr09 | |
| Plurals | `tâches` | fr | fr05, fr06, fr09 | |
| Inflections | `planifier` | fr | fr06, fr07, fr08, fr09, fr11 | |
| Inflections | `planifiés` | fr | fr06, fr07, fr08, fr09, fr11 | |
| Inflections | `relancer` | fr | fr10 | |
| Inflections | `échouées` | fr | fr01, fr10 | |
| Identifiers | `ERR-4012` | en | en02, fr02 | en03 |
| Identifiers | `max_retries` | en | en01, fr01 | en04 |
| Identifiers | `job-id` | en | en05, fr03 | en06 |
| Identifiers | `AgentPort` | en | en07, fr04 | en08 |
| Identifiers | `agentport` | en | en07, fr04 | en08 |
| Topic | `how are failed jobs retried` | en | en11 | |
| Topic | `comment les travaux échoués sont-ils relancés` | fr | fr10 | |
| Topic | `travaux echoues relances` | fr | fr10 | |

A recall check (accents, plurals, inflections) passes when every expected
passage matches. An identifier or topic check passes when every expected
passage scores strictly above every other passage: scores, not ranks, because
Qdrant orders tied scores arbitrarily. The spike's harness was a throwaway;
T040 turns this sample and its checks into Rust tests.
