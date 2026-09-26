//! The lexical analyzer of the BM25 route, profile `bm25-en-fr/1`: it turns a
//! passage or a query into the sparse vector Qdrant searches. Maestro computes
//! these vectors itself, because Qdrant's own BM25 cannot hold a French and
//! English policy (research R7); Qdrant keeps the sparse index, the IDF
//! (`modifier: idf`) and fusion.
//!
//! No language is given or guessed, for a passage or a query: every text goes
//! through the same rules, French and English together.
//!
//! # Terms
//!
//! [`terms`] reads a text in four stages.
//!
//! 1. **Folding.** Combining marks, U+0300 to U+036F, are dropped, and each
//!    precomposed letter of Latin-1 Supplement and Latin Extended-A that has a
//!    canonical decomposition, 161 letters, is written as its base letter,
//!    case kept, so a letter folds alike precomposed and decomposed (NFC and
//!    NFD): `É` is `E` and `ñ` is `n`. The ligatures `œ` and `æ` and the
//!    typographic ligatures `ﬀ`, `ﬁ`, `ﬂ`, `ﬃ` and `ﬄ` are written as their
//!    letters: `Œ` is `OE` and `ﬁ` is `fi`. Every later rule reads folded
//!    text, so a text and the same text without its accents give the same
//!    terms by construction: `tâche` and `tache` both give `tach`.
//! 2. **Identifiers.** A chunk is a run of letters, digits and the joiners
//!    `-`, `_`, `.`, `/`, `\` and `:`; any other character, an apostrophe
//!    among them, ends it, and joiners at either end are trimmed, so
//!    `(job-id)` and `ERR-4012:` read `job-id` and `ERR-4012`, and
//!    `l'exécution` reads `l` and `exécution`. A chunk that still holds a
//!    joiner is an identifier: it gives its whole form, lowercased and nothing
//!    more, then each of its runs as a word, so `max_retries` gives
//!    `max_retries`, `max` and `retri`, `max retries` only the last two, and
//!    the Windows path `D:\data\app.log` gives `d:\data\app.log`, `data`,
//!    `app` and `log`.
//! 3. **camelCase.** A part of a word starts where a lowercase letter or a
//!    digit meets an uppercase letter (`AgentPort`, `base64Encoder`), and
//!    before the last letter of three uppercase letters or more followed by
//!    two lowercase ones (`HTTPServer`), but not in `IDs`, `OAuth` or `ETag`.
//!    A word with parts gives itself, then each part, all as words: `AgentPort`
//!    gives `agentport`, `agent` and `port`, and `agentport` gives `agentport`.
//!    A word longer than 64 characters, such as a base64 blob, gives only
//!    itself.
//! 4. **Words.** An acronym's plural, two capitals or more followed by one
//!    lowercase `s`, loses that `s`: `PDFs`, `CPUs` and `VMs` meet `PDF`,
//!    `CPU` and `VM`, but `Bus` and `Ms` keep theirs. Then a word is
//!    lowercased, dropped if it is a stopword, and stemmed.
//!
//! The stopwords are two short lists written for this profile, an English one
//! and a French one, of articles, pronouns, prepositions, conjunctions and
//! common auxiliaries, looked up after folding (`à`, `où`, `été`) and before
//! stemming. An apostrophe ends a word, so the forms English contractions
//! leave (`don` of `don't`, `isn`, `ll`, `re`, `ve`) are stopwords, as the
//! French elided forms (`l`, `d`, `qu`) are. So are the negation words `not`,
//! `no`, `ne`, `n` and `pas`: a bag of words cannot use negation, which the
//! dense route and the reranker carry. The adverb `not` then gives no term,
//! while `note`, whose stem is `not`, still gives one. A text made only of
//! stopwords has no terms, and so an empty vector.
//!
//! # Stemming
//!
//! Every rule is a general rule of French or English and reads folded text, so
//! none can tell `planifiée` from `planifiee`. A word holding a digit is kept
//! as it is (`ipv4s`). Any other word passes five steps in order, each
//! applying the first of its rules that fits. A rule that drops an ending
//! "when a stem stays" drops it only if three letters or more stay, a vowel
//! (`y` included) among them.
//!
//! 1. **Plurals.** An `x` after `eau` or `eu` goes: `réseaux` meets `réseau`
//!    and `jeux` meets `jeu`, but `index` keeps its `x`. Else `aux` becomes
//!    `al`: `journaux` meets `journal`, while `réseaux`, taken by the rule
//!    before, does not become `reseal`. Else a final `s` goes, unless the word
//!    ends in `ss` or `us`, if a vowel comes before the letter before it:
//!    `jobs` meets `job`, `IDs` meets `id` and `retries` gives `retrie`, but
//!    `class`, `status` and `gas` keep their `s`.
//! 2. **English endings.** A final `ied` becomes `ie`, which step 3 turns
//!    into `i` when a stem stays, as Porter2 does: `tied` meets `tie` and
//!    `ties`, and `died` meets `die`, while `cried` still meets `cry` and
//!    `cries` at `cri`. Else `ed`, else `ing`, goes when a stem stays: `failed`
//!    and `failing` meet `fail`, but `need` and `string` stay whole. When that
//!    leaves `eed`, its `ed` goes too when a stem stays, as it does from the
//!    bare verb: `exceeded` and `exceeding` meet `exceed` and `exceeds` at
//!    `exc`, and `speeding` meets `speed` at `spe`; but `needed` keeps `need`,
//!    as `need` does, `receded`, which leaves no `eed`, keeps `reced`, as
//!    `recede` does, and `agreed` still meets `agree` at `agr`.
//! 3. **French endings.** `ee`, else `e`, goes when a stem stays: the feminine
//!    and the folded `é` of a past participle, so `planifiée`, `planifiées`,
//!    `planifié` and `planifiés` meet at `planifi`, `tâche` gives `tach` and
//!    `retrie` gives `retri`; but `one` stays whole, and `idée` gives `ide`,
//!    not `id`.
//! 4. **Infinitives.** `er` goes when a stem stays: `planifier` meets
//!    `planifié` and `relancer` meets `relancés`, as the English `worker`
//!    meets `work`; but `hier` and `user` stay whole.
//! 5. **Final `y`.** A `y` after a consonant becomes `i`: `retry` meets
//!    `retries` and `retried` at `retri`, and `policy` meets `policies`; but
//!    `day` and `key` keep their `y`.
//!
//! When steps 2 to 4 drop an ending, a final `bb`, `dd`, `gg`, `mm`, `nn`,
//! `pp`, `rr` or `tt` then loses a letter if three letters stay: `logged` and
//! `logging` meet `log`, `planned` meets `plan` and `programme` meets
//! `program`; but `installed` keeps its `ll`, `stuffed` its `ff`, as `stuff`
//! does, `planted` its `nt`, and `added` stays `add`.
//!
//! # Vectors
//!
//! A [`Passage`] counts its terms: its [`term_count`](Passage::term_count) is
//! the length BM25 sets against the average length of a generation's
//! passages, which the caller computes over every passage first
//! ([`AverageLength`]). A passage's term that occurs `tf` times weighs BM25's
//! term-frequency part, `tf·(k1+1) / (tf + k1·(1 − b + b·len/avg_len))`, with
//! k1 1.2 and b 0.75, and Qdrant multiplies it by the term's IDF. A query's
//! term weighs 1, however often it occurs ([`query_vector`]). A term's token ID
//! is the first four bytes of its SHA-256, big-endian. A vector's IDs are
//! sorted and unique, as Qdrant requires, and two terms that share an ID add
//! their weights. The same text always gives the same vector, bit for bit.
//!
//! # Known limits
//!
//! Light rules on folded text merge some words that differ and miss some forms
//! that belong together. The ladder measures the route alone on the golden set
//! (FR-S1-005a), and better rules are a new profile version.
//!
//! - **Words that differ but meet.** A three-letter stem and the French
//!   endings merge English agent nouns with their verbs, and short words with
//!   others, within and across the two languages: `server` and `serve`
//!   (`serv`), `timer` and `time` (`tim`), `header` and `head`, `done` and
//!   `données` (`don`), `state` and `stats` (`stat`), `file` and French `fil`,
//!   `porter`, `porte` and `port`, `news` and `new`, French `relier` and
//!   English `rely` (`reli`), French `pied` and English `pie`.
//! - **French forms that miss each other.** Plurals in `-aux` of words not in
//!   `-al`: `travaux` gives `traval`, not `travail`, and `noyaux` gives
//!   `noyal`, not `noyau`. The feminine of `-el`, since `ll` stays doubled:
//!   `virtuelle`, `optionnelle` and `réelle` miss `virtuel`, `optionnel` and
//!   `réel`. Masculine plurals in `-us`, since `status` keeps its `s`: `menus`,
//!   `obtenus`, `prévus` and `contenus`. Participles in `-is`: `requis` gives
//!   `requi` but `requise` gives `requis`. Verbs in `-ir`: `définir` misses
//!   `défini`. A doubled `ff`: `cheffe` misses `chef`. Derivations:
//!   `planification` misses `planifier`.
//! - **English forms that miss each other.** `alias` gives `alia` but
//!   `aliases` gives `alias`, and `analysis` gives `analysi` but `analyses`
//!   gives `analys`; `embed` gives `emb` but `embedded` gives `embed`; `use`,
//!   `used` and `using` keep their endings, since a two-letter stem is too
//!   short; `reffed` misses `ref`, since `ff` stays doubled.
//! - **Long runs.** The 64-character limit applies to each word, so a long
//!   blob that `/` or `.` cuts into shorter runs still gives their camelCase
//!   parts.
//!
//! # Versions
//!
//! These rules are the profile [`PROFILE`], `bm25-en-fr/1`, which a generation
//! records with its vectors, and a query is analyzed with the profile of the
//! generation it searches. A golden test pins the terms and vectors of sample
//! passages and queries: a rule change that breaks it is a new profile
//! version, `bm25-en-fr/2`, with a golden of its own, never a new golden under
//! the same name.

mod analyzer;
mod fold;
mod split;
mod stem;
mod stopwords;
#[cfg(test)]
mod tests;
mod vector;

pub use analyzer::{PROFILE, terms};
pub use vector::{AverageLength, Passage, SparseVector, query_vector};
