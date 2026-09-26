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

## R8 The cost of reranking on the card

**Question:** what does reranking 80 to 120 pairs cost when they go to the
reranker in one batched call, and which depth leaves room for retrieval inside
SC-S1-004's 1.5 s?

**Answer: about 11 ms a pair, however the call is batched; the ladder starts
at depth 80.** A `/v1/rerank` call is batched on the wire only: the `rerank`
entry runs one slot (`np = 1`), so `llama-server` scores the documents of a
call one after another, and a pair costs the same at 20, 80 and 120
documents. At 120 documents the p95 is 1.41 to 1.42 s, which leaves under
0.1 s for retrieval; at 80 it is 0.90 to 1.13 s. With 0.3 s at p95 allowed for the
rest of a search, 80 is the deepest measured depth that fits. The ladder
measures again on the real corpus, so 80 is where it starts, not a setting.

Measured by T008 on 2026-09-26 on the reference workstation:

| Part | What ran |
| --- | --- |
| GPU | NVIDIA GeForce RTX 5090, 32,607 MiB, Windows driver 616.56, under WSL2 |
| Router | `maestro-model-router` 0.1.0, build `a6dcaeddfeac` (its `/props`) |
| llama.cpp | `build_info` `b1-77f132c` (the reranker's `/props`) |
| Reranker | the catalog's `rerank` entry: `bge-reranker-v2-m3-Q8_0.gguf`; context, batch and micro-batch 8192; flash attention; one slot; 1,190 MiB held |
| Embedder, for the comparison below | the catalog's `embed` entry: `bge-m3-q8_0.gguf`, the same flags with CLS pooling |
| CPU, for the comparison below | AMD Ryzen 7 9800X3D (8 cores, 16 threads); WSL2 sees 8 CPUs |

### Free room refuses rather than unloads

Before any load the router was idle: no request in its journal for 15
minutes, and nothing loaded but two guests, `qwen3-06b` and `embed`, which
the redeploy's free-room check and T018's parity run had loaded. The card had
23,011 MiB free while the router held 4,943 MiB, so about 4.2 GiB was held
outside the router. Admission wants a model's whole estimate free on the
device, and with both guests gone the card would have had about 27,950 MiB:
`turbo38` (30,464 MiB), `qwen38`, `qwen38-semantic` and `heretic38`
(29,184), `qwopus38` and `turbo38-long` (28,928) and `ornith15` (28,672)
would all have been refused. `qwen38-uncensored` (27,648) was the largest
chat entry that could load, so the check used it:

| UTC | Request | Answer | Loaded afterwards |
| --- | --- | --- | --- |
| 14:06:16 | `POST /models/load` for `qwen38-uncensored`, no header | 200 in 18.5 s; the two idle guests unloaded first | `qwen38-uncensored`, 26,857 MiB; the card 1,098 MiB free |
| 14:06:41 | `/v1/embeddings` on `embed`, free room, as a search would | 200 in 1.8 s; the embedder loaded as a guest | the same and `embed`; the card 476 MiB free |
| 14:06:43 | `/v1/rerank` on `rerank`, free room | **503 `insufficient_room`** in 0.07 s | the same two |
| 14:07:10 | `POST /models/unload` for `qwen38-uncensored`, the check's own load, which the journal showed nobody else had used | 200 | `embed` |
| 14:07:16 | the same `/v1/rerank`, free room | 200 in 1.82 s, the reranker loaded cold | `embed`, `rerank` |

The card's free memory is `nvidia-smi`'s reading after each step; admission
reads the device again at each decision. The refusal names what it spared:
"'rerank' was asked for with 'X-Model-Router-Room: free', and there is no
free room for it: loading it would unload embed; nothing was unloaded". So
beside a chat model this large, free room held the embedder but not the
reranker too, as the plan's risk foresaw: a search at that moment runs the
dense route and flags reranking unavailable (FR-S1-015a).

### Method

- **Documents:** public text only, from T014's synthetic collection
  (`tests/fixtures/synthetic/corpus`), whose 152 heading-led sections were
  taken in path order. Each of 120 documents starts at its own section and
  joins the sections after it until it holds 500 tokens, then drops trailing
  words until it holds at most 500, counted with `/models/rerank/tokenize`:
  478 to 500 tokens, median 500, no two alike. With the query and the
  separators, a pair is 505 to 527 tokens, 526 on average.
- **Query:** the synthetic suite's `max-attempts-retries`, 24 tokens.
- **Calls:** one call per measurement, with the first 20, 80 or 120
  documents, and `X-Model-Router-Room: free` on every request:

  ```sh
  curl http://127.0.0.1:8080/models/rerank/v1/rerank \
    -H 'X-Model-Router-Room: free' -H 'Content-Type: application/json' \
    -d '{"model": "rerank", "query": "A job sets maxAttempts to 4: how many times is it retried after its first run fails?", "documents": ["…", "…"]}'
  ```

  Every answer was checked to hold one result per document, mapped back by
  index.
- **Runs:** per depth, one warm-up call, then 30 timed calls, in the order
  20, 80, 120; the whole pass twice.
- **Time:** the client's wall time for the whole exchange through the router,
  over loopback. The p50 and p95 are nearest-rank: the 15th and the 29th of
  the 30 sorted runs.
- **The machine:** other lanes compiled at the same time. The load average
  went from 1.9 to 3.2 during the first pass and from 4.0 to 4.7 during the
  second, on 8 CPUs; the reranker computes on the card, but `llama-server`
  feeds it from the CPU.

The harness was a throwaway script, not committed; what it did is above, and
what it measured is below.

### Results

| Documents | Pass | p50 | p95 | Fastest | Slowest | Warm-up | p50 per pair |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 20 | 1 | 245 ms | 427 ms | 205 ms | 481 ms | 758 ms | 12.3 ms |
| 20 | 2 | 222 ms | 288 ms | 198 ms | 292 ms | 378 ms | 11.1 ms |
| 80 | 1 | 875 ms | 1,132 ms | 776 ms | 1,570 ms | 1,269 ms | 10.9 ms |
| 80 | 2 | 830 ms | 896 ms | 792 ms | 902 ms | 887 ms | 10.4 ms |
| 120 | 1 | 1,324 ms | 1,420 ms | 1,242 ms | 1,454 ms | 1,278 ms | 11.0 ms |
| 120 | 2 | 1,321 ms | 1,407 ms | 1,223 ms | 1,611 ms | 1,264 ms | 11.0 ms |

- **Per pair:** from 20 to 120 documents the p50 grows by 10.8 ms a pair in
  the first pass and 11.0 ms in the second, and a call costs under 30 ms
  besides its pairs. The catalog's 12 ms a pair holds, and batching does not
  lower it: the reranker's log shows one task per document, each ending about
  10 ms after the one before.
- **Tokens:** the router's answers counted 10,517, 42,096 and 63,113 prompt
  tokens at 20, 80 and 120 documents, about 526 a pair.
- **Cold load, apart:** the first call after room was freed loaded the
  reranker and answered in 1.82 s; the router logged `rerank: ready in 1.6
  s`. A search that waits for the load is counted apart (SC-S1-004). The
  first full call after the load was slower too: the first pass's warm-up at
  20 documents took 758 ms against a p50 of 245 ms.
- **The tail:** the first pass is slower in its tail. At 20 documents its
  last eight runs took 298 to 481 ms, and at 80 three runs took over 1.1 s;
  the second pass has no run above 292 ms at 20 or 902 ms at 80. Other lanes
  were compiling throughout.

The runs, in milliseconds, in the order they ran:

```text
pass 1, 14:07:43 to 14:09:02 UTC
20:  275 260 216 228 236 273 226 242 233 277 217 205 276 219 258 219 269 234 245 230 218 221 328 423 375 298 312 481 427 410
80:  1124 998 928 864 882 875 991 1570 1132 959 854 802 778 842 824 872 848 906 882 921 882 879 889 865 840 801 821 820 776 936
120: 1295 1269 1274 1398 1341 1341 1275 1343 1306 1339 1245 1252 1328 1281 1454 1380 1242 1359 1302 1324 1335 1406 1308 1347 1275 1411 1420 1356 1290 1268
pass 2, 14:10:07 to 14:11:21 UTC
20:  217 198 251 292 222 212 237 219 236 204 252 228 229 212 217 252 220 258 230 210 259 231 264 216 217 217 288 257 216 221
80:  822 836 812 848 810 810 830 839 896 864 830 859 801 823 876 839 817 837 846 902 843 882 818 805 792 855 805 794 891 812
120: 1322 1323 1244 1352 1300 1407 1315 1407 1367 1341 1338 1287 1330 1380 1294 1381 1321 1295 1298 1374 1275 1300 1275 1355 1327 1284 1315 1223 1301 1611
```

### The depth

SC-S1-004 wants `knowledge_search` under 1.5 s at p95 with the search models
loaded. This assumes **0.3 s at p95 for everything but reranking**: embedding
the query (4 ms at p50 on the card, below), the routes running in parallel
against Qdrant and SQLite, fusion, evidence assembly and the MCP transport.
T029 measures the routes, and the ladder the whole search.

That leaves reranking 1.2 s at p95: **depth 80 fits** (0.90 and 1.13 s) and
depth 120 does not (1.41 and 1.42 s). The margin at 80 is thin, 70 ms in the
slower pass. Each pair adds about 11 ms, so each 0.1 s more that retrieval
needs costs about 9 pairs of depth.

### On the CPU

Asked by the owner: can embedding and reranking run on the CPU? Measured the
same day, after the card, with the router's own `llama-server` binary started
outside the router on a spare loopback port, one server at a time: the
`embed` entry's GGUF file and flags, then the `rerank` entry's, each with
`--n-gpu-layers 0 --device none` in place of `--n-gpu-layers 999`. This build
carries CUDA, and with `-ngl 0` alone it still offloads host tensor
operations to the card by default (`--op-offload`); with no device, the
card's memory did not move while they ran. The card's embedding figures come
from the router in free room, one input per call, like the reranking.

`llama-server` took all 8 CPUs while other lanes compiled: the load average
went from 4.9 to 9.1 during the embedding runs and from 5.2 to 11.9 during
the reranking runs, so part of the CPU's tail is that contention. With 10 runs,
the p95 is the slowest run.

| Job | Runs | Card p50 | Card p95 | CPU p50 | CPU p95 |
| --- | --- | --- | --- | --- | --- |
| Embed the query, 24 tokens | 30 | 4.0 ms | 21.6 ms | 106 ms | 622 ms |
| Embed one 500-token chunk | 30 | 11.2 ms | 14.9 ms | 1,256 ms | 1,895 ms |
| Embed 50,000 chunks, one at a time, at the p50 | | 9.3 min | | 17.4 h | |
| Rerank 20 pairs | card 30, CPU 10 | 222 to 245 ms | 288 to 427 ms | 16.4 s | 35.9 s |
| Rerank 80 pairs | card 30, CPU 10 | 830 to 875 ms | 896 to 1,132 ms | 105.6 s | 175.9 s |

The card's first eight query runs took 17 to 73 ms and make its p95; the
other 22 took 3.8 to 4.2 ms.

```text
card, embedding, 14:11:52 to 14:11:54 UTC, ms
query: 73.0 21.6 20.7 20.4 20.9 20.5 20.3 16.6 3.8 3.9 4.0 4.1 3.9 4.1 4.0 3.9 3.9 4.1 4.2 4.0 3.9 4.2 4.0 4.0 4.2 4.1 3.8 3.9 3.9 4.0
chunk: 23.5 11.7 11.7 11.1 11.1 10.9 11.8 11.4 11.2 11.6 11.3 11.1 10.6 11.2 11.2 11.3 11.2 14.9 11.2 12.0 11.2 11.2 11.4 11.3 11.2 11.1 11.3 11.1 11.2 11.5
CPU, embedding, 14:12:30 to 14:13:19 UTC, ms
query: 238 116 79 77 85 81 294 1611 86 208 106 175 87 157 65 622 91 82 122 136 337 59 535 57 56 60 461 264 354 51
chunk: 1278 961 1301 1805 1256 879 1211 1116 1136 1196 1488 1440 1070 892 1035 1075 1338 852 1083 1266 964 1196 1374 2979 1739 1739 1895 1764 1612 1461
CPU, reranking, 14:14:04 to 14:40:52 UTC, s
20 pairs: 29.7 27.5 35.9 17.0 16.4 18.9 15.7 16.3 15.2 15.8
80 pairs: 103.2 133.5 100.4 97.0 105.6 122.2 175.9 159.2 173.6 104.6
```

Where each job should run:

- **Query embedding:** on the card, at 4 ms. The CPU answers in 0.1 s at p50
  and 0.6 s at p95 under load: slow beside the card, but fast enough to stand
  in when free room refuses the embedder.
- **Bulk embedding:** on the card: 9 minutes for 50,000 chunks against 17
  hours on the CPU.
- **Reranking:** on the card only. On the CPU, 20 pairs take 16 s at p50,
  more than ten times the whole search budget, and 80 take 106 s.
