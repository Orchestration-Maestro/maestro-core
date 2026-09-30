# S6 research: Rust-first, adaptive acquisition

**Revision 2.2 — 2026-09-30:** Records the owner's tool/policy approvals and browser amendment: crawl4ai is the sole non-Rust browser adapter, Python out of process only where JavaScript/Chromium is unavoidable; Spider's chromey production route is replaced. The document bake-off candidates are unchanged. Adds the validated native asset-identity requirement and the later same-day approval of automatically learned source-evidenced term-alias candidates; OA4c and OA4d remain pending. These are decisions/design fixes, not new measurements.

**Revision 2.1: source register completed (P24–P28, D5); no conclusion changed.**

**Revision 2 — 2026-09-30:** incorporates the supervisor's P1-1–P1-5 and P2-6–P2-9 ruling: pure-Rust extraction/inference candidates, offline-build traps, the S1 ingestion seam and identity continuity, a closed adaptation allow-list, and explicit owner decisions. Published manifests and the affected S1 paths were rechecked; no build, benchmark or protected crawl was run.

**Recommend a Rust-owned acquisition engine, not a new ingestion stack:** use Spider for crawling with extras off, crawl4ai behind the replaceable fetch/render port only for unavoidable browser work, selectors plus htmd checked against Xberg's converter for technical HTML, and the unchanged pinned Xberg versus docling.rs extraction bake-off. Feed the existing S1 canonicalization, deduplication, structural chunking, model router and publication pipeline. Make adaptation a manifest-bounded choice among qualified profiles—not permission to install software, widen access, rewrite evidence or publish unverified output.

**Status:** source research complete; owner tool directions are approved as recorded in §8. Exact dependency/artifact audits, the bake-off result and quality qualification remain open. No protected site was contacted, no credentials or browser databases were opened, and no conversion or retrieval benchmark was run for this document fix. The earlier private mechanism report is separate and was not read in this fix round.

## 1. Evidence and boundaries

All external sources below were accessed **2026-09-30**. Registry versions and publication dates are observations, not recommended automatic upgrades. GitHub default branches and documentation can change; lock an exact release, feature set, model digest and configuration before qualification.

- **Observed:** public registry metadata, published crate manifests, upstream documentation, local architecture and S1 source inspection.
- **Upstream measured:** numerical results reported by a project's authors; not independently reproduced here.
- **Proposed/inferred:** the recommendations and adaptive design below. Platform portability is not a passing Maestro build.
- **Not established:** corpus-wide extraction parity, cross-platform browser/session parity, current dependency deltas, extraction performance, or end-to-end retrieval improvement.

Local references use `maestro-core/` as the root. S1 code and ADR-0020 were read at **`0204846f7640e7219021b0b38daa60b4c4f98ee4`**, the locally available `origin/feat/s1-integration`; this was not fetched or modified. Architecture files were read from the main checkout at **`60b9bd2acbc4315f8dc71d106b9ef92896fc51f5`**. The distinction matters: the main checkout does not contain all S1 implementation.

The controlling contracts are architecture `01-knowledge-pipeline.md:114-383`, roadmap `06-roadmap.md:176-188`, ADR-0013 `:5-23`, and ADR-0020 `:5-20`. The constitution points to these authorities rather than superseding them (`.specify/memory/constitution.md:3-6,25-35`). Source-specific findings, endpoint details and historical measurements remain private.

### What “native” can honestly mean

**Prefer a pure-Rust extraction/inference path, not merely a Rust wrapper around Python or C++.** PDF rendering, layout detection and OCR all have pure-Rust candidates: Xberg's native PDF stack, tract-backed layout, its Rust OCR backends, and standalone ocrs/RTen. Include docling's `pdf-text` without ML as a pure-Rust text baseline. ONNX is a model format, not a requirement to use the C++ ONNX Runtime. [P5, P6, P24–P27]

Within the owner-approved content-processing direction (2026-09-30), the non-Rust exceptions are **crawl4ai (Python, out of process) plus Chromium only for unavoidable browser rendering** and **dynamically loaded ONNX Runtime only for selected table models**. crawl4ai cannot own extraction, frontier state or publication; it replaces Spider's chromey route behind the admitted transport port. Other non-Rust choices are optional, not fundamental: docling's full ML pipeline brings PDFium/ONNX Runtime; Tesseract OCR and KeePassXC credential access are separately declared alternatives. Xberg also documents an ORT-only PP-DocLayout-V3 layout model: choose the tract-supported RT-DETR path by default rather than extending the exception. A feature named `*-tract` is not proof that its entire dependency closure is pure Rust; §3.3 records a Tesseract edge in `paddle-ocr-tract`. Qualify exact features, models and binaries, and never silently relax the pure-Rust/default-offline goal. [P1, P5, P6, P17, P27]

“Any type, any structure” should mean **every authorized input gets a typed, accountable outcome and new formats can plug in without changing the core**. It cannot mean a finite parser perfectly understands every future, encrypted, corrupt or proprietary format. Retain supported authorized bytes; distinguish accepted, needs-reextraction, unsupported, encrypted/locked, corrupt, policy-denied and quarantined outcomes. Unknown data must never quietly become empty accepted Markdown. This is the proposed implementation of the owner's adaptive requirement, consistent with the existing quality gate (`01:365-402`).

## 2. Recommended stage map

### Acquisition and session stages

| Stage | Recommendation | Alternative / qualification condition |
|---|---|---|
| Discovery and frontier | Core-owned durable frontier; enumerate approved seeds, sitemaps, platform APIs and repository refs. Spider fetches one leased unit or a strictly bounded batch; it must not become a second authority over visited state. | Crawlberg is another Rust crawler, but adds no established Maestro advantage. crawl4ai is approved only as the out-of-process browser adapter, never a second producer/frontier. Require complete partition accounting, denial precedence, crash recovery and cancellation. [P1, P2, P7; `01:136-157,211-271`] |
| HTTP | Reuse the workspace reqwest line, minimum features; manual policy-controlled redirects, streaming body limits and representation-aware validators. | Do not duplicate a crawler's HTTP/session state in a parallel client. A header-spoofing client is not a Chromium network stack. [P8; `01:159-174`] |
| JavaScript and browser-stack requests | **Owner amendment, 2026-09-30:** crawl4ai, Python out of process, behind the admitted fetch/render port where a browser is unavoidable. Spider's chromey route is replaced; Spider remains the Rust crawler with extras off. | Pin one production browser route. Digest-bound acquisition profiles select transport/adapter/capabilities and bounded declarative readiness, not network-idle alone. Browser-stack requests must use actual Chromium and pass the same controls, or refuse. No chromey/chromiumoxide fallback; missing containment/interception blocks qualification. [P1, P2, P27; `01:159-174,192-208`] |
| Sessions / authentication | A small session port: anonymous, reuse-session, renew, blocked-auth, challenge and human-needed states. Probe before reading credentials; one account-bound credential pair; in-memory secrets; an explicitly leased browser profile. | Reuse the architecture's read-only KeePassXC CLI binding first. A direct `keepass` adapter is optional, not a reason to port cookie decryption. OS keyring integration is a separate adapter. See §6. [`01:176-208`; P17–P19] |
| Robots | RFC 9309 semantics plus Maestro's stricter fail-closed policy and recorded per-source overrides. Evaluate `texting_robots` against conformance fixtures before adoption. | Library claims do not establish redirect, outage or policy behavior. Do not inherit a crawler's “ignore robots” switch. [P20; `01:246-286`] |

### Extraction stages

| Input | Recommendation | Boundary / alternative |
|---|---|---|
| Technical HTML | Preserve captured HTML; site-profile selectors first, htmd with tested code/table/admonition/link handlers. dom_smoothie only as an optional content-selection fallback whose output passes the same fidelity gates. | Readability can discard short prerequisites and examples. Never feed its flattened text into the converter. `html-to-markdown-rs` is a permissively licensed bake-off alternative; `html2md` is GPL-3.0+ and not the default recommendation. [P9–P11] |
| Markdown / plain text | Preserve original Markdown; directly reuse S1's pulldown-cmark canonicalizer. For plain text, a typed text adapter must preserve exact code/line semantics and unknown structure. | No HTML or PDF converter round-trip; no invented headings. [`crates/maestro-canonicalization/src/pipeline.rs:18-106`] |
| PDF, DOCX, PPTX and other document formats | First bake-off: **Xberg 1.3.0** minimal format features versus **`docling` 1.78.0** with a `pdf-text` no-ML baseline and separately qualified ML profile. Select one qualified path per media/profile, with bounded fallback on structural loss. | `docling-rs` 0.1.2 is a Docling Serve SDK, not the Rust converter. Keep Python Docling comparison-only, never a production extraction exception. Simple PDF text crates are not table/layout substitutes. [P5, P6, P12–P15] |
| Spreadsheets | calamine for cell values and separately read formulas; preserve sheet/cell coordinates, cached value versus formula, and warnings about missing formatting. | It does not calculate formulas or reproduce full spreadsheet presentation. Reject encrypted/unsupported elements honestly; do not run macros or external references. Xberg's spreadsheet support can be compared behind the same contract. [P5, P16] |
| Images, audio, video, attachments and archives | Detect and dispatch each authorized member independently. OCR/transcription/visual interpretation are opt-in derived profiles with provenance and uncertainty. ZIP/TAR adapters enforce path, link, depth, entry, expansion, time and total-byte limits before extraction. | Keep originals and parent/member lineage. No archive auto-execution; no installer extraction without separate approval. Other archive formats need an installed qualified adapter or an unsupported outcome. [P5, P6, P21; `01:224-244,314-334`] |

Structured inputs are also explicit profiles: JSON/YAML/XML/CSV should use typed normalizers that retain keys, values, ordering where meaningful and source locations, rather than a general document converter. Repository source code stays source code; the separate S7 code-intelligence pipeline is not replaced by acquisition-time prose conversion. Email/e-books and additional binary formats can use an engine only after that specific format/profile is qualified; a long engine format list is not blanket support. [`01:321-324`; P5, P6]

### Wiki-specific mapping, not one generic scraper

| Platform | Prefer | Required mapping and limitation |
|---|---|---|
| Confluence Cloud | REST v2 pages and attachment APIs; opaque cursor/next-link pagination. | Preserve page ID, parent/space, version, selected body representation and attachments; evaluate effective caller visibility and restrictions. Space permissions alone are not a complete page ACL. Cloud and Data Center require different tested adapters. [W1] |
| MediaWiki | Action API allpages/revisions plus recentchanges; copy the complete continuation object. | Preserve page/revision IDs, namespace, timestamps, redirect targets and file revisions. An empty batch is not necessarily end-of-stream; overlap incremental windows. Rendered output and source wikitext are different representations; unsupported templates remain visible findings. [W2] |
| Other wikis | Product API/export before scraping; declared HTML fallback only if qualified. | Each connector must supply identity, revisions, hierarchy, pagination, attachments, permissions and withdrawal semantics. Do not advertise a wiki as supported solely because its HTML can be downloaded. [`01:317-319`] |

## 3. Candidate due diligence

### How to read the register

For every Rust row, its linked crates.io page is the release source. The precise machine-readable sources used were `https://crates.io/api/v1/crates/<name>` (version, licence, publication timestamp, repository) and `https://crates.io/api/v1/crates/<name>/owners` (publisher accounts). **Owners are publishing authorities, not a measured bus factor or an assurance of support.** Dates are UTC dates of the listed stable version. Native/platform/quality notes immediately below are part of each candidate's assessment.

ADR fit codes: **R** = reuse existing functionality/version where possible; **Q** = owner-approved qualification needed; **B** = benchmark alternative only; **X** = reject as default / does not meet the native goal. Even R is not approval to upgrade. All Q/B rows require a fresh minimal-feature dependency measurement and licence/native-link audit (§3.8).

### 3.1 Crawl and browser candidates

| Candidate | Licence | Stable release / date | Registry owners | Fit |
|---|---|---|---|---|
| [spider](https://crates.io/crates/spider/2.53.9) | MIT | 2.53.9 / 2026-09-05 | madeindjs, j-mendez | Q, named in ADR-0020 |
| [chromiumoxide](https://crates.io/crates/chromiumoxide/0.9.1) | MIT OR Apache-2.0 | 0.9.1 / 2026-02-25 | mattsse, Sytten | B, historical alternative, not the approved browser route |
| [fantoccini](https://crates.io/crates/fantoccini/0.22.1) | MIT OR Apache-2.0 | 0.22.1 / 2026-02-28 | jonhoo | B |
| [crawlberg](https://crates.io/crates/crawlberg/1.8.0) | MIT | 1.8.0 / 2026-09-27 | Goldziher | B |
| [reqwest](https://crates.io/crates/reqwest/0.13.5) | MIT OR Apache-2.0 | 0.13.5 / 2026-09-08 | seanmonstar | R |

The following upstream capability facts are retained; they do not override the 2026-09-30 crawl4ai-only browser decision. Spider and Crawlberg are Rust implementations; web-page rendering adds Chromium, not a pure-Rust browser. Spider 2.53.9's `chrome` feature selects `chromey` 2.54, its own chromiumoxide fork. A separate chromiumoxide fallback therefore adds a second CDP stack, and docling's optional `web-browser` adds `headless_chrome`, a third. The approved production profile enables none of these Rust CDP stacks; crawl4ai supplies the sole browser route. Spider's defaults include `io_uring`, `numa` and `splice`, whose implementations are Linux-targeted (NUMA becomes a no-op elsewhere), plus other features; use `default-features = false` and pin a minimal cross-platform fetch profile rather than inherit these defaults. [P27] chromiumoxide is a Rust CDP client; Fantoccini is a Rust WebDriver client with an external driver/browser. reqwest is Rust with TLS/platform-native dependencies determined by features. Windows/macOS/Linux are deployment candidates for these combinations, but this lane did not test any of them; qualify the actual browser and driver builds on each OS. chromiumoxide documents both launching and attaching to Chromium and optional downloading: **disable implicit downloads** in production. [P1–P4, P7, P8]

Maturity: Spider/reqwest have established release histories; browser compatibility remains a moving dependency. chromiumoxide/Fantoccini remain 0.x APIs. Crawlberg is a newer alternative and has no local parity evidence. No candidate's marketing throughput establishes documentation fidelity. The architecture records a narrow earlier selected-HTML parity result, but it does not establish a platform matrix, policy interception or general extraction quality (`01:169-174`). Vendor-specific details are not repeated here.

**Crawl4AI:** [v0.9.4, 2026-09-23](https://github.com/unclecode/crawl4ai/releases/tag/v0.9.4), UncleCode/project contributors; Python plus browser automation, not Rust-native. Its [licence](https://github.com/unclecode/crawl4ai/blob/main/LICENSE) contains Apache-2.0 text **and an additional attribution section**; review the actual distribution terms. Upstream supports browser-based crawling, but W/M/L packaging must be independently qualified. **Owner-selected 2026-09-30 as the sole non-Rust browser adapter**, confined to unavoidable render/browser operations behind the admitted port, not a production extractor or independent crawler. Record a named ADR-0020 exception, pinned Python/browser/adapter distribution and licence evidence; its other crawling/extraction/model features are disabled. Existing legacy comparison use remains distinct. No corpus-wide quality number or transport-security qualification was measured here. [P2]

### 3.2 HTML and Markdown candidates

| Candidate | Licence | Stable release / date | Registry owners | Fit |
|---|---|---|---|---|
| [htmd](https://crates.io/crates/htmd/0.5.5) | Apache-2.0 | 0.5.5 / 2026-07-27 | letmutex | Q, preferred HTML conversion |
| [dom_smoothie](https://crates.io/crates/dom_smoothie/0.18.2) | MIT | 0.18.2 / 2026-09-21 | niklak | Q, optional fallback |
| [html-to-markdown-rs](https://crates.io/crates/html-to-markdown-rs/3.15.1) | MIT | 3.15.1 / 2026-09-27 | Goldziher, tobocop2 | B |
| [html2md](https://crates.io/crates/html2md/0.2.17) | GPL-3.0+ | 0.2.17 / 2026-08-20 | Kaned1as | X absent licence decision |
| [pulldown-cmark](https://crates.io/crates/pulldown-cmark/0.13.4) | MIT | 0.13.4 / 2026-05-20 | marcusklaas, raphlinus, Martin1887 | R, already canonicalizer |

These are Rust HTML/Markdown processing libraries; no Python or browser is required for their conversion role. W/M/L portability is expected from these paths, **not verified here**. htmd exposes custom tag handlers, which fits technical-document fidelity. dom_smoothie follows Mozilla Readability and explicitly warns that formatted text does not preserve table structure; its README also describes cases where readability candidate selection drops content. The alternate HTML converter reports broad capabilities, not a Maestro gold-set win. No independent documentation benchmark was identified in the primary sources inspected for these candidates. Use structure and exact-code fixtures, not string similarity alone. htmd/dom_smoothie are evolving 0.x interfaces; pin them. [P9–P11; S1 `pipeline.rs:14-16`]

Xberg 1.3.0's `pdf-native` path enables its `html` feature and therefore `html-to-markdown-rs`. If Xberg wins, adopting htmd introduces a **second HTML converter**, not the first. Include both in the same technical-HTML gold-set comparison; retain htmd only when its handlers provide demonstrated fidelity or policy benefits. Do not chain the converters. [P5, P11, P27]

### 3.3 General document engines and PDF alternatives

| Candidate | Licence | Stable release / date | Registry owners | Fit |
|---|---|---|---|---|
| [xberg](https://crates.io/crates/xberg/1.3.0) | MIT | 1.3.0 / 2026-09-29 | Goldziher, tobocop2 | Q, first bake-off candidate |
| [kreuzberg](https://crates.io/crates/kreuzberg/4.10.4) | MIT at this release | 4.10.4 / 2026-09-21 | Goldziher | B, legacy line |
| [docling](https://crates.io/crates/docling/1.78.0) | MIT | 1.78.0 / 2026-09-30 | artiz | Q, native port bake-off |
| [docling-rs](https://crates.io/crates/docling-rs/0.1.2) | MIT | 0.1.2 / 2026-02-17 | mohammedsafvan | X, HTTP SDK not converter |
| [pdf-extract](https://crates.io/crates/pdf-extract/0.12.1) | MIT | 0.12.1 / 2026-09-16 | jrmuizel | B, simple PDF baseline |

| Candidate | Licence | Stable release / date | Registry owners | Fit |
|---|---|---|---|---|
| [lopdf](https://crates.io/crates/lopdf/0.45.0) | MIT | 0.45.0 / 2026-09-08 | J-F-Liu | B or engine transitive dependency |
| [pdfium-render](https://crates.io/crates/pdfium-render/0.9.4) | MIT OR Apache-2.0; separate PDFium notices | 0.9.4 / 2026-09-06 | ajrcarey | B, explicit native backend |
| [calamine](https://crates.io/crates/calamine/0.36.1) | MIT | 0.36.1 / 2026-07-27 | tafia, jmcnamara, Expurple | Q, spreadsheets |

#### Pure-Rust inference / OCR candidates

| Candidate | Licence | Stable release / date | Registry owners | Fit |
|---|---|---|---|---|
| [tract](https://crates.io/crates/tract/0.23.8) | MIT OR Apache-2.0 | 0.23.8 / 2026-09-21 | kali | Q, preferred CPU inference family |
| [tract-onnx](https://crates.io/crates/tract-onnx/0.23.8) | MIT OR Apache-2.0 | 0.23.8 / 2026-09-21 | kali | Q, Xberg's ONNX backend dependency |
| [ocrs](https://crates.io/crates/ocrs/0.13.1) | MIT OR Apache-2.0 | 0.13.1 / 2026-09-13 | robertknight | B, standalone Rust OCR |
| [rten](https://crates.io/crates/rten/0.26.0) | MIT OR Apache-2.0 | 0.26.0 / 2026-08-29 | robertknight | B, ocrs inference runtime |

tract is a Rust inference project with Linux/macOS/Windows paths; Xberg uses `tract-onnx`, so do not add a second direct API dependency without need. Pin CPU features and the actual model/operator set: support for ONNX does not imply support for every ONNX graph. ocrs and RTen offer Rust OCR/inference without Tesseract or ONNX Runtime; W/M/L portability is a qualification target, not a test result here. ocrs describes itself as early preview and currently Latin-alphabet-only, with more errors than commercial OCR; this is not a general multilingual replacement. Its CLI downloads models on first use—use the library with preprovisioned models instead. No local documentation OCR comparison or three-OS qualification has run. [P24, P25]

#### Additional PDF benchmark candidates

| Candidate | Licence | Stable release / date | Registry owners | Fit |
|---|---|---|---|---|
| [pdf_oxide](https://crates.io/crates/pdf_oxide/0.3.78) | MIT OR Apache-2.0 | 0.3.78 / 2026-09-08 | yfedoseev | B, Rust extraction/rendering alternative |
| [oxidize-pdf](https://crates.io/crates/oxidize-pdf/5.2.0) | MIT | 5.2.0 / 2026-09-29 | bzsanti | B, Rust PDF alternative |
| [hayro](https://crates.io/crates/hayro/0.7.1) | Apache-2.0 OR MIT | 0.7.1 / 2026-06-05 | LaurenzV | B, pure-Rust PDF rasterizer |

These are benchmark-only, not additional mandatory production engines. pdf_oxide offers Rust text/Markdown extraction and rendering; selected OCR/GPU/table features can add tract, ONNX Runtime or PDFium, so qualify the minimal feature closure. Its public throughput/parse-pass benchmark is not proof of ordered-cell or exact-code fidelity. oxidize-pdf is Rust with optional Tesseract OCR; version 5.x alone does not demonstrate extraction parity. hayro is an explicitly experimental Rust interpreter/renderer with documented unsupported PDF cases; rendering is not semantic table extraction. Their Rust paths make W/M/L plausible, but all three need actual platform, hostile-file and technical-document tests. No local results exist. [P26]

**Xberg is the renamed successor, not merely a guessed spelling of Kreuzberg.** The published 1.3.0 manifest has small defaults (`tokio-runtime`, `simd-utf8`); `pdf` selects `pdf-native`, with `pdf-pdfium` separate. `xberg-native-pdf` has an all-Rust rendering/codec dependency design: `tiny-skia`, `skrifa`, `harfrust`, the `hayro-jbig2`/`hayro-jpeg2000` codecs and Rust compression, rather than PDFium. The manifest does not directly depend on the standalone `hayro` crate. Full resolved graphs still need audit for feature unification. [P5, P27]

Its pure-Rust inference candidates are `layout-tract` (RT-DETR and table-type classification), `sceptre-ocr-tract`, and CPU `candle-ocr`; `paddle-ocr-tract` also selects tract inference, **but 1.3.0 transitively enables `ocr` and thus `xberg-tesseract`**. It is not an entirely C/C++-free feature closure until that edge is avoided or fixed. Do not enable `layout-tract` together with the ORT `layout-detection` profile. The published manifest says TATR/SLANeXT table-structure recognition and PP-DocLayout-V3 are ORT-only; use tract-supported layout first and restrict any initial ORT exception to needed table models. Do not turn on `full`: it enables unrelated networking, embedding, chunking and hosted-model features. [P27]

Xberg's current project targets W/M/L; its layout guide documents CPU, Windows/Linux CUDA and macOS CoreML paths. Qualify CPU first and each accelerator separately. Legacy Kreuzberg has a separate support/release line; do not infer the licence or dependency graph of an older Kreuzberg release from today's MIT successor. The project's [changelog](https://github.com/xberg-io/xberg/blob/main/CHANGELOG.md) records licence changes. Legacy PDF/OCR configurations can include PDFium/Tesseract/ONNX; they require their own pinned audit. [P5]

The native port's Cargo name is **`docling`**, repository **docling-project/docling.rs**. Its 1.78.0 default feature set is `pdf`, `asr`, `fetch-images`, `vlm`; do not adopt those defaults. **`default-features = false` plus `pdf-text`** enables docling-pdf without ML: use this lopdf-based pure-Rust text path as a separate bake-off baseline, not as a claim of layout/OCR/table-model parity. The full `pdf` feature enables `docling-pdf/ml`, adding PDFium and ONNX inference. The Windows guide documents MSVC and provisioned PDFium/models; Linux/macOS are documented paths, but no Maestro three-OS qualification occurred. Optional browser pre-render is explicitly untested on Windows there. [P6, P12, P27]

**Maturity facts:** the docling.rs repository was created **2026-06-27**; the registry snapshot contains **193 published versions** and **one publisher (`artiz`)**. Fast release volume in a young repository is not mature extraction parity. Freeze the candidate and record upstream fixes rather than following each release during the bake-off. Repository creation and registry observations are dated 2026-09-30. [P28]

**Offline-build traps:** Xberg's `layout-detection` and `onnx-runtime` enable `ort-bundled`, which enables `ort/download-binaries`. docling-pdf's optional ORT dependency unconditionally requests `download-binaries` whenever that dependency is enabled by ML; `pdf-text` avoids it. Disabling a top-level crate's defaults does not remove these feature edges. Xberg's `ort-dynamic` enables dynamic loading, and upstream documents that it can short-circuit downloading, but Cargo features remain additive: it does **not remove** `download-binaries` from the resolved feature graph. To meet the recommended strict rule—dynamic ORT, **never `download-binaries`**—require an upstream feature split or approved audited patch before qualifying those ORT profiles; do not claim the unmodified profiles already meet it. Test both build and execution with network blocked and approved model/runtime assets provisioned in advance. [P27]

**Upstream measured, not ours:** docling.rs's current PDF conformance document reports **9/18 byte-exact and 10/18 whitespace-normalized** against a particular Python groundtruth and renderer/model configuration. It lists remaining table, reading-order and heading differences. Those numbers are not a general extraction accuracy score, and the best comparison renderer/configuration is not automatically the default native deployment. Xberg's layout guide reports **structure F1 33.9% → 41.1%, text F1 87.4% → 90.1%**, with **447 → 1500 ms/document** on its **171-document CPU corpus**. These are author-reported, different corpora and metrics: **do not rank the two engines by comparing those numbers**. [P12, A4]

`pdf-extract` and lopdf are Rust PDF text/object tools, portable W/M/L in principle; neither is evidence of full layout/OCR/table reconstruction. `pdfium-render` wraps C++ PDFium; major desktop builds are available separately. Its README warns that PDFium calls are serialized for thread safety and records memory-safety fixes in 0.9.4. Isolate hostile-file processing and pin the binary ABI; do not assume Rust wrapper safety removes native-parser risk. calamine is pure Rust and portable in principle, but its documentation explicitly excludes much formatting and encrypted content. No local fidelity result exists for these three alternatives or calamine. [P13–P16]

The `docling-rs` SDK's Rust client can be portable, but the server owns conversion and its dependencies; this does not retire Python. Python Docling remains an MIT-licensed project comparator, not a new native dependency. Its exact legacy installed version must be captured in each parity receipt rather than silently upgraded to the latest release. [P6, P22]

### 3.4 Detection and normalization candidates

| Candidate | Licence | Stable release / date | Registry owners | Fit |
|---|---|---|---|---|
| [infer](https://crates.io/crates/infer/0.22.0) | MIT | 0.22.0 / 2026-07-15 | bojand | Q if not already covered by selected engine |
| [tree_magic_mini](https://crates.io/crates/tree_magic_mini/3.2.2) | MIT code; optional embedded MIME database GPL | 3.2.2 / 2025-11-14 | mbrubeck | B, database/licence complication |
| [whatlang](https://crates.io/crates/whatlang/0.18.0) | MIT | 0.18.0 / 2025-10-16 | greyblake | R, already a workspace dependency |
| [unicode-normalization](https://crates.io/crates/unicode-normalization/0.1.25) | MIT OR Apache-2.0 | 0.1.25 / 2025-10-30 | huonw, SimonSapin, Manishearth, kwantam, sujayakar | R/Q, derived text only |
| [texting_robots](https://crates.io/crates/texting_robots/0.2.2) | MIT OR Apache-2.0 | 0.2.2 / 2023-03-29 | Smerity | Q, standards test required |

These have Rust implementations, with W/M/L portability inferred rather than exercised. `infer` uses signatures without an external magic database; it cannot prove that arbitrary text is Markdown or that a valid header means the whole file is safe. `tree_magic_mini` defaults to loading system MIME data, which undermines reproducibility unless pinned; its own docs warn that embedding its GPL database changes licence obligations. Prefer `infer` or the selected engine's already-present detector over adding both. whatlang exposes language/confidence/reliability; short commands, mixed-language documents and code need an unknown result rather than a forced language. Unicode normalization must never mutate preserved evidence or code. `texting_robots` has an older last published release: lack of recent releases alone is not a defect, but RFC fixtures and dependency audit remain unperformed. No documentation-quality score applies to these helpers in isolation. [D1–D4, P20; `01:359-363`]

Xberg 1.3.0 already depends on `infer` 0.22 unconditionally: if it is selected, reuse that resolved detector rather than count it as a new independent addition. whatlang is already declared at `crates/maestro-knowledge/Cargo.toml:24` in the inspected S1 commit. [P27]

| Conditional candidate | Licence | Stable release / date | Registry owners | Fit |
|---|---|---|---|---|
| [lingua](https://crates.io/crates/lingua/1.8.0) | Apache-2.0 | 1.8.0 / 2026-03-09 | pemistahl | B, only if mixed-language accuracy fails |

lingua is a Rust language detector with multilingual/short-text ambitions and selectable language-model features. It is not an automatic replacement for already-installed whatlang: first measure failures on the actual mixed-language/code corpus, then compare accuracy, abstention, memory and model footprint. W/M/L portability is expected for the Rust path but untested here; upstream benchmarks do not establish Maestro accuracy. [D5]

### 3.5 Archives and secret handling candidates

| Candidate | Licence | Stable release / date | Registry owners | Fit |
|---|---|---|---|---|
| [zip](https://crates.io/crates/zip/8.6.0) | MIT | 8.6.0 / 2026-04-25 | Plecra, Pr0methean | Q/reuse transitive version |
| [tar](https://crates.io/crates/tar/0.4.46) | MIT OR Apache-2.0 | 0.4.46 / 2026-05-18 | cgwalters | Q/reuse transitive version |
| [keyring](https://crates.io/crates/keyring/4.2.0) | MIT OR Apache-2.0 | 4.2.0 / 2026-08-29 | hwchen, brotskydotcom | Q, optional secure-store adapter |
| [keepass](https://crates.io/crates/keepass/0.15.0) | MIT | 0.15.0 / 2026-09-21 | sseemayer, louib | B, direct vault adapter |
| [secrecy](https://crates.io/crates/secrecy/0.10.3) | Apache-2.0 OR MIT | 0.10.3 / 2024-10-09 | tony-iqlusion | Q/reuse, secret wrapper |

| Candidate | Licence | Stable release / date | Registry owners | Fit |
|---|---|---|---|---|
| [zeroize](https://crates.io/crates/zeroize/1.9.0) | Apache-2.0 OR MIT | 1.9.0 / 2026-06-12 | tarcieri, github:rustcrypto:utils | Q/reuse, best-effort memory hygiene |

ZIP/TAR parsing is Rust; codec features can introduce native libraries. Select only required codecs, never assume archive unpacking is safe because a helper has an extraction method. keyring deliberately calls platform secure stores; W/M/L backends differ and Linux requires an available configured store. The keepass crate reads KDBX in Rust, but unlock factors and file variants need fixture qualification. secrecy/zeroize protect accidental exposure and memory lifetime; they do not guarantee erasure of every copy, OS swap, subprocess buffer or browser-managed secret. Archive/security correctness is the relevant quality criterion here, not documentation retrieval scores. All six need real target and security tests; none ran here. [P18, P19, P21, P23]

**KeePassXC CLI:** external C++/Qt application, [2.7.12 released 2026-03-10](https://github.com/keepassxreboot/keepassxc/releases/tag/2.7.12), KeePassXC project. Its [COPYING](https://github.com/keepassxreboot/keepassxc/blob/2.7.12/COPYING) documents mixed component licences; do not treat this separately distributed program as a permissively licensed Rust crate. W/M/L is the project deployment scope; qualify the selected executable and unlock method. Reuse its read-only CLI mechanism rather than its legacy plaintext unlock-file practice. It has no document extraction quality score. [P17]

### 3.6 Quality conclusions

No library qualifies “perfect ingestion” by itself. Upstream technical fixtures, format counts, release counts and download popularity are not task-level fidelity. The only numerical extraction evidence cited here is explicitly upstream-reported; the current corpus bake-off has not run. Library selection should follow the profile-specific acceptance tests in §7 and §8, not marketing format totals.

### 3.7 Pure-Rust baseline and explicit native exceptions

| Content-processing profile | Runtime / assets to declare |
|---|---|
| HTML + Xberg native PDF / docling `pdf-text` | Rust conversion/rendering or text-only paths; Xberg native PDF uses Rust graphics/font/codec crates, while docling `pdf-text` provides text without ML. Neither requires PDFium or ONNX Runtime for that role. Audit TLS/codec feature closure separately. |
| Rust layout and OCR | tract / `tract-onnx`, Xberg `layout-tract`, `sceptre-ocr-tract`, CPU `candle-ocr`, or standalone ocrs/RTen with local weights. `paddle-ocr-tract` has a Tesseract feature edge in 1.3.0, so is not yet a qualified pure-Rust bundle. |
| Browser fetch/render | Owner-approved crawl4ai (Python, out of process) plus Chromium, only for unavoidable browser work. No Spider chromey path or alternative non-Rust browser adapter. Pin adapter/runtime/browser artifacts, sandbox and flags; qualify checked-address egress and bounded declarative readiness behind the common transport port. |
| ONNX-only table-structure profile | ONNX Runtime loaded dynamically against a pinned local library; no `download-binaries`. Requires resolving the published feature-edge traps in §3.3. This exception is per model, not the layout/OCR default. |
| Optional full docling ML comparator | PDFium, ONNX Runtime and layout/table/OCR weights; not the pure-Rust baseline. Its unconditional ML dependency download feature must be corrected before an offline-build qualification. |

Credential and archive choices are separate: existing KeePassXC is an explicitly external C++ tool; OS secure stores and any native archive codecs must be declared. These do not establish a need for C++ in PDF rendering, layout or OCR. The table is **not a complete SBOM**. Check each model-weight licence and provenance before the bake-off; pin weight, runtime and conversion artifacts by digest and provision offline. A Rust/MIT engine does not confer its licence on the weights it loads. [P3, P5, P6, P12, P17–P21, P24–P27]

### 3.8 ADR-0020 adoption gates

ADR-0020 already records a **2026-09-26, 128-crate baseline**, not today's workspace: dom_smoothie 0.18.2 added 45 crates with one forced duplicate; docling 1.69.2's minimum/default trees added 120–363 with 9–23 duplicates; Spider 2.53.9 added 186–331 with 15–38 duplicates. Spider defaults also forced an incompatible second SQLite `links` dependency (`docs/adr/0020-…:22-34`). Those historical measurements must not be reused as measurements of Xberg, docling 1.78.0 or the current S1 lock.

Before adoption: resolve the exact minimal profile against the integration lock; record added packages, forced duplicates and native `links`; name every DEP-001 exception with its removal condition; obtain owner library approval; audit licences and `cargo vet`; build/test on Linux, Windows and macOS with network-disabled extraction and provisioned assets. No dependency graph was resolved or compiled here, so the new counts are **unknown**, not zero. An out-of-process adapter can isolate a native-link conflict, but does not waive these obligations or make process separation a sandbox. [ADR-0020:16-20,46-57; ADR-0013:5-23]

## 4. Reuse S1 from canonical blocks through embeddings

### Already implemented: do not duplicate

| Function | Observed implementation and what to reuse |
|---|---|
| Canonical structure and provenance | `crates/maestro-canonicalization/src/pipeline.rs:18-106` preserves Markdown, accounts for parser gaps and calculates content/provenance revisions. `model.rs:124-156` already accepts extractor blocks, original locations, Markdown spans and structured JSON. This is library-level support, **not the current import seam**: S6 must bridge the missing fields as described in item 1 below, rather than invent a second canonical model. |
| Exact and near dedup | `dedup.rs:24-34,47-120` keeps authorized occurrences separate and verifies exact representations. `crates/maestro-knowledge/src/prepare/near.rs:1-16,26-33,89-108` implements 128-hash MinHash, 32×4 LSH, exact 5-shingle Jaccard confirmation at 0.85, and connected groups. Reuse; a transitive group does not mean every pair meets the threshold. |
| Structural chunks and groups | `prepare/chunking.rs:1-9,58-95` calls the existing chunker using the model tokenizer. `maestro-canonicalization/src/unit_graph/groups.rs:1-55` provides page/section/table/procedure/code ancestry and version-family fields. Preserve primary spans and nonembedded parent context; no Xberg/Docling chunker in series. |
| Embedding | `prepare/bridge.rs:1-16,103-107` reuses the model port for tokenization; `index/dense.rs:35-75` formats model input, bounds the call and validates count/dimensions/finite nonzero vectors. Do not enable an extractor's embedded embedding service. |
| Publication aliases | `index/publish.rs:66-97` verifies a generation, moves the projection alias and records publication/recovery behavior. Reuse this atomic publication route, never let an extractor publish directly. |
| Sparse / lexical index | `crates/maestro-knowledge/src/index/sparse.rs:1-40` computes BM25 passage statistics/vectors using the existing `lexical/` analyzer; `lexical/analyzer.rs:8-38` supplies `bm25-en-fr/1`, term analysis and acronym singularization. Reuse alongside dense embeddings; do not add an extractor-owned sparse index. |

### What S6 must add or connect

1. **Capture-to-canonical adapter and the missing S1 seam:** supply faithful Markdown, typed extractor structures, real page/cell/DOM locations, assets and fidelity receipts, with raw capture and derived Markdown distinct. But `crates/maestro-knowledge/src/import/entry.rs:183-186` currently fills only identity and metadata on `CanonicalizeInput`; extractor blocks and assets remain empty. `corpus.rs:33-70` defines `maestro-corpus/1` with no typed fields for them—its opaque `extractor` map does not wire them through. Two options: **(a)** introduce `maestro-corpus/2` entries carrying blocks/locations and assets for portable serialized handoff; **(b)** expose a direct **core-side in-process ingestion call** carrying these fields and reuse import's integrity, identity, revision, hold and journal logic. **Recommend (b)** for native S6: factor the existing transaction path rather than maintain a second importer or require an intermediate corpus export. Out-of-process extensions still submit through the governed core boundary; they are not loaded in-process. Choose (a) if an external/offline producer needs that persistent interchange contract. The spec must choose explicitly, and a mapped PDF/table/image fixture must prove locations and assets survive into stored canonical artifacts. Missing coordinates remain unknown.
2. **Acquisition state:** frontier, leases, policy decisions, approved URL/discard registry, sessions, complete discovery coverage, representation validators, tombstones and repair. Reuse kernel authority/journal/artifacts instead of a second crawler database.
3. **Quality and profile routing:** admit/hold decisions before S1 preparation, drift signals and approved fallback selection; version every change so replay and cache invalidation are reliable.
   **Asset-only revision correction (validation fix):** the current canonicalizer hashes document/content/metadata/extractor blocks (`pipeline.rs:44–52`) but stores assets separately (`:70`); import returns unchanged for an existing revision (`import/entry.rs:112–125`). Native ingestion must bind a versioned semantic inventory digest (stable destination, status, verified content digest/length or explicit unknown) before canonicalization. Use a core-reserved source-metadata key already covered by the revision preimage; exclude transient observations and reject source-supplied collisions. corpus/1 adds no key and retains byte-equal IDs. N26 must prove missing→available and asset-byte-only changes create immutable revisions with every other input equal, and repeated equal inventories remain unchanged. This implements architecture 01's “Markdown or relevant assets” revision rule, not a second importer.
4. **Aliases and identity continuity:** publication aliases already exist; URL/page-ID aliases and replacement/version relationships are distinct. S1 import IDs hash the namespace, collection ID and exact `source_ref` bytes (`import/entry.rs:165-174`): require **byte-equal `source_ref` with the legacy producer per family**, preserving the collection namespace, or an explicit versioned old-ID → new-ID migration. Requested versus final URL, trailing slash and query ordering cannot be normalized silently at cutover. Store alternate references without merging permissions; canonical tags are hints, not identity proof. No general source-alias registry was established. **Alias scope approved by the owner, 2026-09-30:** automatically learn term aliases (other names for the same thing) from documents that explicitly define a short form or variant, with its supporting span. No hand-written per-product list. Candidates are reviewable/reversible, never silently merged; preserve an S2 reviewed `ALIAS_OF`-compatible identity seam without waiting for S2. Search-time query expansion using approved aliases is a later S1 item, not S6 implementation. Source-reference continuity above remains required. S1's `lexical/analyzer.rs:21-38` only singularizes acronym plurals; it is not evidence that this synonym capability already exists.
5. **Withdrawal and converter migration wiring:** current-policy rechecks, permission changes and extractor/profile changes must invalidate affected derived generations while preserving permitted audit provenance. Do not interpret an incomplete enumeration or transient 403 as a confirmed deletion. [`01:136-157,246-305,335-402`]

Normalization should be minimal and representation-specific: preserve commands, indentation, identifiers, versions, negations, units, link destinations and table cell text. Clean table padding without touching code; unwrap presentation tables carefully; keep alt text and asset references. A lossless representation with warnings is preferable to model-rewritten “clean prose.” [Architecture `01:327-363`]

## 5. Adaptive, manifest-configured handling of unknown structure

This section incorporates the owner's added requirement: “any unstructured data, any type, any structure … self configurable within the config manifest and adaptive.” Everything described as a Maestro rule here is **proposed design**, not implemented or benchmarked behavior.

### 5.1 Content-based detection

Use bounded bytes, not extension or Content-Type alone. Record declared MIME, detected candidates, detector/version, confidence or reason, container subtype and mismatches. Prefer an existing selected engine's detector when it supplies the needed independent signals; otherwise `infer` is the smallest new candidate. It needs no magic database. tree_magic_mini is an alternative with a system-data and GPL-embedding complication. Xberg exposes `MimeDetectionPolicy` with `PreferContent`, extension-preference and content-only choices; freeze it explicitly rather than relying on defaults. [D1, D2, A4]

Do shallow signature detection first, then bounded parser validation and ZIP/OLE container inspection. Text has no universal magic signature: distinguish valid encodings, markup/parser success and an unclassified text fallback. Treat HTTP-200 login HTML labelled PDF, polyglots, truncated files, decompression bombs and conflicting detectors as refusals or quarantine, not clever auto-repair. Do not trust archive filenames, document macros, embedded URLs or inferred types as authority to fetch or execute anything.

### 5.2 Structure inference and selection signals

| Signal family | Available mechanism | Honest failure mode / use |
|---|---|---|
| Native structure | HTML DOM, Markdown AST, Office XML, PDF tags/text geometry, wiki block IDs. | Prefer explicit structure; tags and font sizes can be misleading or absent. Maintain source mappings and parser warnings. [P5, P6, P9–P16] |
| Layout, OCR and tables | Prefer Xberg's tract RT-DETR layout and qualified Rust OCR (`sceptre-ocr-tract`, CPU `candle-ocr`, or standalone ocrs/RTen); its native PDF stack supplies Rust rendering. Compare docling `pdf-text` as the no-ML baseline. Reserve dynamically loaded ONNX Runtime for needed ORT-only table models; full docling ML remains an explicit non-Rust comparator. | Pure-Rust inference is model/operator-specific, not universal ONNX support. `paddle-ocr-tract` still pulls Tesseract in 1.3.0; docling ML and Xberg ORT features have build-time download traps (§3.3). Confidence is not content correctness: qualify multicolumn order, merged cells, exact code and model licences with pinned offline assets. [P6, P12, P24, P25, P27, A4] |
| Boilerplate and code | Profile selectors, DOM density/repetition and optional dom_smoothie; preserve `<pre>/<code>`, fences and indentation. Xberg layout includes code and formula classes. | Readability and layout inference can delete prerequisites or misclassify code. Use them to propose regions, not to silently discard uncertain content. [P9, A4] |
| Language and repetition | whatlang on substantial prose regions, with abstention; reuse S1 exact/near dedup and group family keys. | Mixed-language/code-heavy input defeats one document-wide label. Near-duplicate groups are hints for version-family review, never authority to merge or delete. [D3; S1 references in §4] |
| Missing-content evidence | Empty-page rate, replacement characters, source-versus-output headings/code/tables/cells, unresolved attachments, parser errors and mapping gaps. | More extracted characters alone is not better. Structure counts can match while cell contents/order are wrong; combine receipts with exact fixtures and human labels. [`01:345-381`] |

No model can infer a missing entitlement or a source's true revision/version from appearance. Keep unknown metadata unknown. Scanned text, inferred table structure and visual descriptions must retain their method and uncertainty; changing OCR/layout can change answer-critical symbols even when output reads fluently.

### 5.3 What existing adaptive tools actually do

| Tool / approach | Source-backed mechanism | Transferable lesson and failure modes |
|---|---|---|
| Unstructured partition strategies | Its source chooses `hi_res` for requested table/image extraction, `fast` for extractable PDF text, otherwise `ocr_only`; images default to `hi_res`. Missing inference/OCR dependencies trigger alternative strategies or errors. [A1] | Reuse the idea of explicit deterministic routing, not the Python dependency. A fallback may silently lose table fidelity if callers ignore warnings. An extractable text layer can still be wrong. Requires Python plus optional native/ML dependencies; W/M/L and engine support vary. Apache-2.0 project, comparison reference only. |
| Docling options | `PdfPipelineOptions` exposes OCR, table structure, OCR engine and cell-matching choices; advanced options explain when disabling cell matching can fix merged columns. The Rust port has its own features/configuration, not a guarantee that every Python option is identical. [A2, P6] | Auto behavior is constrained pipeline configuration, not universal self-learning. Validate each version, model and backend; do not activate network fetching, VLMs or first-use downloads by default. |
| LlamaParse Auto Mode | Vendor documentation describes standard parsing followed by per-page premium escalation on table/image/text/regex triggers. [A3] | Useful cheap-first/expensive-on-demand pattern. This is a hosted service comparison, not a local Rust implementation or an approved egress route. Vendor examples of diagrams/charts are not independent fidelity scores. Regex triggers can miss novel layouts or over-trigger on logos. No service/SDK adoption is recommended. |
| Xberg | Content-preferred MIME detection, automatic OCR routing and optional layout `auto` screening of geometry; skipped pages and reasons are reported. Layout `always` retains per-page model classification. Plain output discards markup even when layout ran. [A4] | Set output format and OCR/layout permissions explicitly. “Auto” must never invoke an unapproved backend or download models. The layout guide says `auto` can trade away paragraph-level heading/code/formula refinement on skipped pages. |
| Adaptive chunking / statistical drift | A published adaptive-chunking study selects methods using reference completeness, cohesion, context coherence, block integrity and size compliance. ADWIN detects changes in streaming statistics using an adaptive window. [A5, A7] | Both motivate measurement-based selection; neither is proof of ingestion correctness or a ready-made source-policy controller. Begin with deterministic bounded profiles and per-source baselines; consider a statistical detector only after fixed signals show a need. |

Unstructured, Python Docling and LlamaParse are **comparative designs, not candidate production dependencies**. Their code/service licences and packaging do not change the Rust-first recommendation. No latest release claim or deployment qualification is made for these comparison-only tools.

### 5.4 Adaptive chunking: keep the structural baseline

**Evidence is mixed.** “Is Semantic Chunking Worth the Computational Cost?” evaluates document/evidence retrieval and answer generation and finds no consistent benefit justifying semantic chunking's added cost. Conversely, the 2026 adaptive-chunking paper reports improved answer correctness from method selection on its own legal/technical/social-science corpus. These are different experiments, not contradictory universal laws. Late chunking is another distinct approach: contextualize tokens with a long-context encoder before pooling chunk vectors; it needs a different embedding interface and is not a free extractor feature. [A5, A6, A8]

**Proposed default:** S1's existing structural units, mapped spans, table/procedure/code groups and model-token budgets. Select only among prequalified profile IDs based on reliable headings, table/code density, page geometry, language confidence, unit-size distribution and required-context coverage. A malformed/no-heading document can select a qualified conservative text profile; complex tables can select an appropriate mapped table profile. Do not flatten to satisfy a budget or synthesize text to fill a chunk.

Before enabling semantic splitting or a different token budget, compare against that baseline with fixed embedder, tokenizer, reranker, queries and generation settings. Measure evidence-span recall, answer-bearing-unit coverage, retrieval ranking, answer/citation quality, embedding tokens, index size and latency. Stratify table, procedure, code, narrative and mixed-language cases. Lock acceptance thresholds before inspecting holdout results; owner-approved existing quality targets stay fixed. Intrinsic chunk scores alone do not authorize publication. No local retrieval gain is claimed.

### 5.5 Drift detection and safe changes

Track signals **per source/profile/media/language stratum**, not one global average:

| Signal | Likely cause to investigate | Safe response |
|---|---|---|
| Selector miss, main-region shrinkage, navigation ratio, heading-path or DOM-shape shift | Site redesign or wrong page state | Hold suspicious capture; replay selectors and alternative HTML profile on the same immutable capture. |
| Redirect/login/challenge rate, permission probe failure, changed account realm | Session/authorization drift | Stop affected source and renew through normal policy; never switch to a permissive profile. |
| Tables/code/cells lost, replacement symbols, OCR confidence shift, blank or reordered pages | Engine/model/layout/language drift | Run a bounded qualified fallback, retain both attempts, compare structural and content evidence. |
| Changed revision/ETag with identical visible text; new links/assets; changed family distribution | Metadata/link-only change or discovery drift | Reprocess identity/metadata/discovery without relying on text-only dedup; do not delete on partial coverage. |
| Token-size/refusal distribution, mapping gaps, quality dispositions, retrieval canary regressions | Extractor/chunker/configuration regression | Hold candidate generation; compare against the previous qualified profile and roll back selection if needed. |

A proposed change follows **observe → shadow replay → qualify → approve/activate → publish**, not “edit production defaults when a score falls.” Freeze captures, baseline profile, candidate profile, model/artifact digests, sample membership and acceptance rules. Test known good and adversarial fixtures, an unseen holdout and source-family parity modes; run three-OS replay and offline/no-download tests. Record each attempted profile and rejection reason, with bounded attempts and total resources. Rebuild downstream artifacts under new profile identities, verify the generation, then use S1's publication alias. Automatic changes are permitted **only by the closed write allow-list below**; the absence of a field from an illustrative danger list is never permission to change it. [`01:136-157,246-286,345-402`; ADR-0020; S1 `index/publish.rs:66-97`]

**Closed adaptation write allow-list (exhaustive):**

1. Selection among **pre-qualified profile IDs** already in the frozen eligible set.
2. Cleanup rules, subject to unchanged evidence-preservation and fidelity gates.
3. The **S1 chunk strategy within the existing model limit**, without altering that limit or the model/tokenizer.
4. Dedup keys, without changing source/document identity, authorization or the no-delete meaning of near-duplicate groups.
5. **New** `exclude_from_knowledge` or `asset_only` entries; no removal or relaxation of existing entries.

**Everything else is immutable to adaptation.** Hold a proposal touching any other field, even if its author claims it is safer or improves a score; an authorized configuration review, outside the adaptation mechanism, must decide it. This covers profile definitions/eligible sets, binaries/models, licences, grants/credentials, URLs and query-identity rules, `deny_fetch` removals, robots overrides, wiki permission interpretation, budgets, fidelity tolerances, quality targets, drift thresholds and the allow-list itself. Rejected mixed proposals are held as a whole, not partially applied. Allowed changes still need replay and the fixed quality/publication gates; cleanup never rewrites source evidence and dedup never merges access rights.

ADWIN is a possible later detector for numerical rates, not the first implementation. A corpus mix change, a short window, seasonality or sparse source can trigger a statistical alarm without any extraction regression. Detection only nominates investigation; immutable replay and quality gates decide whether a change is safe. [A7]

### 5.6 Pluggable profile registry

Use strict JSON **collection/source configuration** under ADR-0014; leave the existing catalog extension declaration format alone. A source selects an approved acquisition/extraction profile registry. Recommended registry content, not a proposed finalized schema:

| Record group | Minimum meaning |
|---|---|
| Identity / provenance | Profile ID and version, configuration digest, adapter protocol major, executable digest, library features, model and native-binary digests, supported OS/architecture and licence evidence. |
| Match / routing | Supported media/container types, source/structure predicates, language applicability, ordered candidate IDs, explicit default and unknown outcome; deterministic tie/abstention rules. |
| Limits / authority | Input/page/DOM/archive limits, attempts/time/memory/process limits, allowed effects, credential role reference and offline requirement. These narrow existing grants; they never create them. |
| Output / qualification | Extractor contract version, required blocks/locations/fidelity fields, mapping semantics, acceptance profile, fixture/holdout evidence and known unsupported structures. |
| Adaptation / lifecycle | Frozen eligible profiles, routing signals, fallback reasons, drift thresholds, minimum sample policy and rollback state. **Only the five write categories in §5.5 are adaptable**: qualified-profile selection, cleanup rules, S1 chunk strategy within the fixed model limit, dedup keys, and new `exclude_from_knowledge`/`asset_only` entries. Every other field in this registry is immutable to adaptation; hold any proposal touching it. |

Consumers use the plan's small `ProfileRegistry` resolve/select port, not concrete registry storage. Substitute and disabled adapters must pass N15 conformance without caller changes; disabled returns an explicit unavailable result with no trusted fallback or effects. Acquisition transport/adapter/readiness lives in its own digest-bound strict profile (N03/N46), not inferred from extraction-profile selection.

The registry validator enforces the closed allow-list against the complete proposed diff before applying any selection/configuration update. In particular, **Limits / authority**, **Identity / provenance**, **Output / qualification**, eligibility definitions and adaptation-control fields are not writable by adaptation. Core-generated audit receipts/configuration digests record accepted changes; they are not a back door for the adapting component to edit frozen configuration.

The Rust core owns policy, frontier, artifact storage, selection receipts and publication. Third-party/private adapters are out-of-process extensions using ADR-0013's versioned Operations API/events and existing JSON-RPC lifecycle, not Rust dynamic libraries. Extractors receive bounded authorized artifacts, not unrestricted URLs or credentials; network is off unless an explicit connector operation needs it. Outputs remain untrusted until core validation. Process separation does not replace OS sandboxing. [ADR-0013:5-23; `07-extensibility.md:138-152,184-208`]

**Scheduling gap to resolve in the implementation plan:** S6 depends on S1, while the general daemon extension host is assigned to S4 (`06:22`; `07:254-261`). Do not quietly make S6 depend on a completed S4 or duplicate its full host. Qualify a minimal S6 launcher against the same process protocol and security contract, with a documented later host handoff; if required containment is unavailable on a target, refuse the adapter rather than relaxing it. This is a recommendation, not an assertion that the launcher already exists.

## 6. Safe session design

The private report contains the legacy mechanism evidence. The public design should expose no vendor endpoint, account, cookie name or collection fact.

1. **Bind one principal:** source → auth role → one account/realm → vault/session binding. Anonymous sources never inspect credentials. Retrieve a whole pair atomically, not missing fields from unrelated providers. Test a synthetic second collection and wrong-account refusal.
2. **Reuse, then renew:** validate browser/profile compatibility and ownership; probe a protected target under the authorized preflight budget. Retrieve credentials only after expiry is established. Use a normal SSO browser flow; MFA or legal attestation pauses for a person where required.
3. **No insecure secret storage:** unlock material from an operator prompt, secure OS store or inherited protected channel, never a plaintext file, manifest, argv or log. CLI invocation is read-only, timeout-bounded, with stdin unlock and redacted stderr. Avoid environment credentials when a protected channel exists. Keep cookie/token values in memory; protect owned browser-profile storage through OS controls, and never serialize a plaintext session jar into artifacts.
4. **Browser ownership is explicit:** no killing or resetting another user's profile. Do not port a platform-specific legacy cookie cipher as a universal Rust authentication method. If importing an existing store is necessary, implement a platform-qualified read-only adapter with exact domain/account matching, snapshot consistency and keyring checks; otherwise request operator sign-in in the owned profile.
5. **Bounded failure:** challenges, wrong account, missing binding, busy profile, robots denial and failed renewal become typed outcomes. Stop before repeated MFA/login loops. Never log full redirected/signed URLs, authorization headers, storage state or child-process credential output. [`01:176-208`; P17–P19, P23]

The architecture's first protected preflight is at most **180 seconds, two protected reads, one login cycle, retries disabled**; broad live work has separate approved budgets. This lane performed neither. Browser traffic still requires destination/subresource policy and credential isolation; rendering success is not an egress-security test. [`01:203-208,283-286`]

## 7. Proving parity and preventing regression

**Retire one source family at a time, not Python globally on the strength of a happy-path page.** Roadmap `06:178-188` requires fixtures and a sampled live diff covering full, incremental, resume, withdrawal and repair, then independent review and native scheduled refresh.

### 7.1 Offline evidence matrix

**Identity gate for every family:** native and legacy producers must emit **byte-equal `source_ref`** under the same collection namespace, or the cutover must carry an explicitly approved, versioned old-ID → new-ID migration. S1 derives IDs from the namespace, collection and exact `source_ref` (`crates/maestro-knowledge/src/import/entry.rs:165-174`). Compare document-ID sets as well as content: requested/final URL substitution, trailing-slash changes and query reordering can otherwise duplicate or withdraw identical documents. Test continuity on unchanged content and metadata-only revisions; migration must preserve occurrence provenance, holds, permissions and rollback mapping.

| Family | Fixture and comparison obligation |
|---|---|
| Documentation HTML and wiki | Selected/raw/rendered representation labels; headings, exact code, admonitions, nested/spanning tables, links/anchors, images, hierarchy, revision IDs and permissions. Include layout redesign, JS shell, login HTML and API pagination. |
| Knowledge articles and community | Every discovered ID reaches fetch; all admitted object types; edited/deleted replies and parent relationships; unchanged text with changed HTML/links; complete capped-partition enumeration. |
| Repositories | Exact repo/ref/commit, permitted paths, unsafe archive entries, no implicit submodules/LFS, missing/corrupt local tree at unchanged commit, atomic replacement and deletion. |
| Catalogue and binary assets | Stable catalogue identity, checkpoint versus accepted snapshot, empty signed URL, expiry/remint, Range/ETag/length/digest mismatches, missing owned file, disk exhaustion, partial transfers and asset-only disposition. |
| Local files, attachments, Office/PDF and other media | Extension/MIME disagreement, embedded assets, page/sheet/slide/member coordinates, encrypted/corrupt/unsupported inputs, OCR/layout errors, code/table loss, archive bombs and converter/profile changes. |

Compare **facts and structure**, not just Markdown bytes: stable identities, authorized item coverage, state transitions, permissions, attachments, raw digest where representation matches, exact code/parameter/value content, ordered table cells, source mappings and quality outcomes. Record both false omissions and false additions. A better native result may legitimately differ from defective legacy output; approve a fixture-backed explained delta rather than reproducing a known bug.

For live qualification, obtain explicit protected-access authorization first, pass the bounded preflight, then select a reproducible stratified sample under an approved broader budget. Pin old/new configurations; match snapshot/revision/time windows where possible; record source changes between reads as uncertainty, not parser failure. Never claim an API snapshot where none exists. Sampling size and acceptance tolerances are owner decisions based on risk and source mix, **not numbers invented by this lane**. A missing family or unresolved critical structure/permission loss blocks that family's cutover.

### 7.2 The twelve mandatory historical regressions

Architecture `01:288-307` lists twelve defects. Each needs a synthetic failing test before the native connector ships; group below without dropping any:

| Test IDs | Defect → required native behavior |
|---|---|
| R1, R2, R3 | New discovery not queued → explicit handoff; incremental checkpoint overwrites accepted catalogue → separate immutable accepted state; failures marked visited/zero exit → retryable failed states and truthful run result. |
| R4, R5, R6 | Incomplete walk called complete/watermark advanced → partition accounting and deferred watermark; curated community types/reply edits/deletions omitted → complete supported-object reconciliation; text-only equality hides HTML/link changes → representation-aware change keys and discovery. |
| R7, R8, R9 | Resume/redirect bypass and missing deny lists → current policy on every request; mutable payload with stale history label → immutable revision artifacts; exclusions/deletions/converter changes leave stale outputs → explicit invalidation/rebuild/withdrawal. |
| R10, R11, R12 | Expired signed URLs/missing owned assets skipped → remint plus integrity-checked repair; unchanged repo commit masks missing/corrupt tree → verify tree integrity; failed prerequisites feed stale downstream stages → dependency completion fencing, no implicit stale fallback. |

Also test code whitespace and special characters, layout tables wrapping content tables, table padding, images without invented descriptions, login/challenge shells, unknown formats, offline model availability, wrong-account secrets, redirects/subresources/private IPs, cancellation and all process-tree resource limits. These are the extraction/security obligations already specified in `01:136-208,246-286,327-363`, not evidence that the tests currently pass.

### 7.3 Publication and cutover receipt

One immutable receipt records source family, fixture and live-sample IDs, legacy/native versions, binary/model/configuration digests, policy and exclusion snapshot, selected/rejected profiles, timing/resource limits, coverage and state counts, fidelity/content diffs, unresolved uncertainty, quality/retrieval gate results and independent review. Native scheduled refresh must pass with the Python producer disabled for that family. Keep a rollback to a verified authorized revision; rollback never revives withdrawn permissions. This extends the existing run-receipt and generation contracts rather than inventing a second release mechanism. [`01:155-157`; roadmap `06:185-188`; S1 publication references in §4]

## 8. Owner decisions and next action

### Tools — approved 2026-09-30

The owner answered yes to the following, with one amendment:

1. **Crawl/HTML:** Spider for crawling with extras switched off; htmd for HTML to text, checked against Xberg's built-in converter. Do not chain converters or add optional dom_smoothie/GPL html2md without a separate decision.
2. **Bake-off:** **Xberg 1.x (MIT) against docling-rs on our files, winner adopted.** Keep the research comparison unchanged: pinned Xberg 1.3.0 native PDF/Rust layout/OCR versus the native `docling` 1.78.0 `pdf-text` baseline; full ML remains separately declared comparison only. The owner label means docling.rs, not the `docling-rs` service SDK. No winner is claimed before results. Private-file use still waits for OA4c.
3. **Inference:** **tract by default for layout and OCR models; ONNX Runtime only for table models, loaded dynamically and never auto-downloaded.** Published download feature edges still require a split or approved audited patch; enabling dynamic loading alone is insufficient. [P24, P27]
4. **Weights:** **Pinned, stored offline, each licence checked before use.** Record original source, licence text, conversion/export lineage and local digest; the engine licence is not the model licence.
5. **Amendment, verbatim:** “rust only except we have no choice for js page or chromium use python alternative like crawl4ai”. **crawl4ai (Python, out of process)** is the only non-Rust browser adapter, behind the admitted fetch/render port only when JavaScript/Chromium is unavoidable. It replaces Spider's chromey path, not the Rust producer/extractor/connector. N02 records the named ADR-0020 exception; N46 must prove containment, egress, readiness and owned lifecycle. No fallback to another browser stack is authorized.

### Rules — approved 2026-09-30

| Group | Owner decision | Remaining gate |
| --- | --- | --- |
| OA1 | Q1–Q5 with **δ = 0.01**, the fixed matrix and comparison protocol as proposed in spec. | Freeze actual cohorts/suite/baseline before results; every required gate must pass. |
| OA2 | Internal wikis **one named origin at a time**, owner approval and **expiry**. | N48 architecture amendment plus exact authenticated origin grant and qualified enforcement. |
| OA3 | The spec's operating envelopes **as proposed**, unchanged. | Enforce bounds; variations/resources still need approval, not a capacity claim. |
| OA4a/OA4b | Private vendor connector **one source family at a time**; first preflight **one source/account/target, 180 seconds, two protected reads, one login cycle, zero retries**; **no robots override by default**. | Exact source/account grants remain necessary; exceptional override needs its own scoped receipt. |
| OA5 scope | M6 **file types as proposed** in the required media matrix. | Exact feature/artifact/licence records, observed bake-off winner and platform/profile/model qualification; no universal support claim. |

### Alias — approved 2026-09-30

The owner's later same-day answer chooses **term aliases**, automatically learned
from source definitions of short forms/variants with exact supporting spans.
Candidates remain reviewable/reversible, with no hand-written product list or
silent identity merge. N29 stores scoped evidence and the S2 `ALIAS_OF`-compatible
identity seam; query expansion over approved aliases is a later S1 search item
only. This is approved scope, not an existing implementation or automatic review.

### Still pending

1. **OA4c:** private data and comparison use; no private file read or live diff is authorized by these tool choices.
2. **OA4d:** each family's retirement after evidence and independent review; no blanket cutover.

**Next action:** N02 records the approved shortlist and named crawl4ai exception with exact artifact/feature audits before implementation; the public/synthetic offline bake-off needs no protected login.

## Source register

All links accessed 2026-09-30. Registry/owners endpoints for every candidate are defined in §3; sources below support capabilities, caveats and comparisons. Upstream snapshot identifiers observed: Xberg `c5610d0ce9b1b1f20e4c23682498f23a44412fde`; docling.rs `15d584e379085972f40c07519e52ea0166cbd922`. Published manifests were additionally read from the crates.io release archives for Xberg 1.3.0 and docling 1.78.0; default-branch documentation is not substituted for those manifests.

### Crawl / extraction

- **P1:** [Spider](https://github.com/spider-rs/spider), [published 2.53.9](https://crates.io/crates/spider/2.53.9).
- **P2:** [Crawl4AI README](https://github.com/unclecode/crawl4ai), [licence](https://github.com/unclecode/crawl4ai/blob/main/LICENSE), [0.9.4 release](https://github.com/unclecode/crawl4ai/releases/tag/v0.9.4).
- **P3:** [chromiumoxide README, fetcher and caveats](https://github.com/mattsse/chromiumoxide).
- **P4:** [Fantoccini README](https://github.com/jonhoo/fantoccini).
- **P5:** [Xberg README](https://github.com/xberg-io/xberg), [1.3.0 package](https://crates.io/crates/xberg/1.3.0), [release archive](https://crates.io/api/v1/crates/xberg/1.3.0/download), [Kreuzberg legacy](https://docs.kreuzberg.dev/).
- **P6:** [docling.rs](https://github.com/docling-project/docling.rs), [docling 1.78.0 archive](https://crates.io/api/v1/crates/docling/1.78.0/download), [different docling-rs SDK](https://docs.rs/docling-rs/0.1.2/docling_rs/).
- **P7:** [Crawlberg](https://github.com/xberg-io/crawlberg), [1.8.0 archive](https://crates.io/api/v1/crates/crawlberg/1.8.0/download).
- **P8:** [reqwest 0.13.5 documentation](https://docs.rs/reqwest/0.13.5/reqwest/).
- **P9:** [dom_smoothie README: readability and formatting limits](https://github.com/niklak/dom_smoothie).
- **P10:** [htmd README and handlers](https://github.com/letmutex/htmd).
- **P11:** [html-to-markdown Rust core](https://github.com/xberg-io/html-to-markdown), [html2md licence metadata](https://crates.io/crates/html2md/0.2.17).
- **P12:** [docling.rs PDF conformance](https://github.com/docling-project/docling.rs/blob/master/docs/PDF_CONFORMANCE.md), [Windows deployment](https://github.com/docling-project/docling.rs/blob/master/docs/WINDOWS.md).
- **P13:** [pdf-extract](https://github.com/jrmuizel/pdf-extract).
- **P14:** [lopdf](https://github.com/J-F-Liu/lopdf).
- **P15:** [pdfium-render: runtime binaries, thread safety and release fixes](https://github.com/ajrcarey/pdfium-render).
- **P16:** [calamine: formulas, values and unsupported features](https://github.com/tafia/calamine).
- **P17:** [KeePassXC CLI manual](https://github.com/keepassxreboot/keepassxc/blob/2.7.12/docs/man/keepassxc-cli.1.adoc), [COPYING](https://github.com/keepassxreboot/keepassxc/blob/2.7.12/COPYING).
- **P18:** [keyring secure-store backends](https://github.com/open-source-cooperative/keyring-rs).
- **P19:** [keepass-rs](https://github.com/sseemayer/keepass-rs).
- **P20:** [RFC 9309](https://www.rfc-editor.org/rfc/rfc9309), [texting_robots](https://docs.rs/texting_robots/0.2.2/texting_robots/).
- **P21:** [zip 8.6.0](https://docs.rs/zip/8.6.0/zip/), [tar 0.4.46](https://docs.rs/tar/0.4.46/tar/).
- **P22:** [Python Docling README](https://github.com/docling-project/docling), [licence](https://github.com/docling-project/docling/blob/main/LICENSE).
- **P23:** [secrecy 0.10.3](https://docs.rs/secrecy/0.10.3/secrecy/), [zeroize 1.9.0](https://docs.rs/zeroize/1.9.0/zeroize/).
- **P24:** [tract repository](https://github.com/sonos/tract), [tract 0.23.8 release metadata](https://crates.io/api/v1/crates/tract/0.23.8), [tract-onnx 0.23.8 release metadata](https://crates.io/api/v1/crates/tract-onnx/0.23.8). Accessed 2026-09-30; supports Rust inference, ONNX and documented desktop targets, not universal operator compatibility.
- **P25:** [ocrs repository](https://github.com/robertknight/ocrs), [RTen repository](https://github.com/robertknight/rten), [ocrs 0.13.1 release metadata](https://crates.io/api/v1/crates/ocrs/0.13.1), [rten 0.26.0 release metadata](https://crates.io/api/v1/crates/rten/0.26.0). Accessed 2026-09-30; ocrs README explicitly states early-preview/Latin-only limits, RTen inference and first-use CLI model downloads.
- **P26:** [pdf_oxide repository](https://github.com/yfedoseev/pdf_oxide), [0.3.78 release/features](https://crates.io/api/v1/crates/pdf_oxide/0.3.78); [oxidize-pdf repository](https://github.com/bzsanti/oxidizePdf), [5.2.0 release/features](https://crates.io/api/v1/crates/oxidize-pdf/5.2.0); [hayro repository](https://github.com/LaurenzV/hayro), [0.7.1 release/features](https://crates.io/api/v1/crates/hayro/0.7.1). Accessed 2026-09-30; supports the alternative engine roles, feature-dependent native paths and hayro's experimental status, not Maestro fidelity.
- **P27:** Published release archives and their Cargo manifests: [Xberg 1.3.0](https://crates.io/api/v1/crates/xberg/1.3.0/download), [xberg-native-pdf 1.3.0](https://crates.io/api/v1/crates/xberg-native-pdf/1.3.0/download), [docling 1.78.0](https://crates.io/api/v1/crates/docling/1.78.0/download), [docling-pdf 1.78.0](https://crates.io/api/v1/crates/docling-pdf/1.78.0/download), [Spider 2.53.9](https://crates.io/api/v1/crates/spider/2.53.9/download); [Xberg layout guide](https://docs.xberg.io/guides/layout-detection/) for model/backend support. Accessed 2026-09-30; manifests confirm the native PDF, HTML/detector, browser, OCR/Tesseract and additive ORT download-feature edges described in §3. They do not establish a fully resolved graph or offline execution success. [Kreuzberg 4.8.0 licence metadata](https://crates.io/api/v1/crates/kreuzberg/4.8.0) and [Xberg rename/licence changelog](https://github.com/xberg-io/xberg/blob/main/CHANGELOG.md), also accessed 2026-09-30, support the historical name/licence distinction.
- **P28:** [docling.rs repository metadata](https://api.github.com/repos/docling-project/docling.rs), [docling registry versions](https://crates.io/api/v1/crates/docling), [docling registry owners](https://crates.io/api/v1/crates/docling/owners). Accessed 2026-09-30; recheck observed creation timestamp `2026-06-27T16:51:49Z`, 193 version records and one publisher account (`artiz`). These are dated observations, not maturity scores.

### Detection, wikis and adaptation

- **D1:** [infer signature detector](https://github.com/bojand/infer).
- **D2:** [tree_magic_mini 3.2.2, including MIME database licence warning](https://docs.rs/tree_magic_mini/3.2.2/tree_magic_mini/).
- **D3:** [whatlang](https://github.com/greyblake/whatlang-rs).
- **D4:** [unicode-normalization](https://docs.rs/unicode-normalization/0.1.25/unicode_normalization/).
- **D5:** [lingua-rs repository](https://github.com/pemistahl/lingua-rs), [lingua 1.8.0 release/features](https://crates.io/api/v1/crates/lingua/1.8.0). Accessed 2026-09-30; supports Rust language detection, short-text goals and selectable language features, not a measured win over Maestro's existing detector.
- **W1:** [Confluence REST v2 pagination](https://developer.atlassian.com/cloud/confluence/rest/v2/), [attachments and permissions](https://developer.atlassian.com/cloud/confluence/rest/v2/api-group-attachment/), [space permissions](https://developer.atlassian.com/cloud/confluence/rest/v2/api-group-space-permissions/).
- **W2:** [MediaWiki Allpages](https://www.mediawiki.org/wiki/API:Allpages), [Revisions](https://www.mediawiki.org/wiki/API:Revisions), [RecentChanges](https://www.mediawiki.org/wiki/API:RecentChanges), [Continue](https://www.mediawiki.org/wiki/API:Continue).
- **A1:** [Unstructured strategy-selection source](https://github.com/Unstructured-IO/unstructured/blob/main/unstructured/partition/strategies.py), [licence](https://github.com/Unstructured-IO/unstructured/blob/main/LICENSE.md).
- **A2:** [Docling advanced options](https://docling-project.github.io/docling/usage/advanced_options/), [pipeline options](https://docling-project.github.io/docling/reference/pipeline_options/).
- **A3:** [LlamaParse Auto Mode, vendor description](https://www.llamaindex.ai/blog/optimize-parsing-costs-with-llamaparse-auto-mode).
- **A4:** [Xberg layout detection, upstream benchmark and auto-screening caveats](https://docs.xberg.io/guides/layout-detection/), [type/configuration reference](https://docs.xberg.io/reference/types/).
- **A5:** [Adaptive Chunking: Optimizing Chunking-Method Selection for RAG, 2026](https://arxiv.org/abs/2603.25333).
- **A6:** [Is Semantic Chunking Worth the Computational Cost?, 2024/NAACL 2025](https://arxiv.org/abs/2410.13070), [published paper](https://aclanthology.org/2025.naacl-findings.114/).
- **A7:** [Bifet and Gavaldà, Learning from Time-Changing Data with Adaptive Windowing, SDM 2007](https://epubs.siam.org/doi/10.1137/1.9781611972771.42).
- **A8:** [Late Chunking: Contextual Chunk Embeddings Using Long-Context Embedding Models, 2024](https://arxiv.org/abs/2409.04701).
