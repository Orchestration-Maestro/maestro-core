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
//! 1. **Folding.** Combining marks, U+0300 to U+036F, are dropped, and the
//!    accented letters of French and English and the ligatures `œ` and `æ` are
//!    written in their base letters, case kept: `É` is `E`, `Œ` is `OE`. Every
//!    later rule reads folded text, so a text and the same text without its
//!    accents give the same terms by construction: `tâche` and `tache` both
//!    give `tach`.
//! 2. **Identifiers.** A chunk is a run of letters, digits and the joiners
//!    `-`, `_`, `.`, `/` and `:`; any other character, an apostrophe among
//!    them, ends it, and joiners at either end are trimmed, so `(job-id)` and
//!    `ERR-4012:` read `job-id` and `ERR-4012`, and `l'exécution` reads `l`
//!    and `exécution`. A chunk that still holds a joiner is an identifier: it
//!    gives its whole form, lowercased and nothing more, then each of its runs
//!    as a word, so `max_retries` gives `max_retries`, `max` and `retri`, and
//!    `max retries` only the last two.
//! 3. **camelCase.** A part of a word starts where a lowercase letter or a
//!    digit meets an uppercase letter (`AgentPort`, `base64Encoder`), and
//!    before the last letter of three uppercase letters or more followed by
//!    two lowercase ones (`HTTPServer`), but not in `IDs`, `OAuth` or `ETag`.
//!    A word with parts gives itself, then each part, all as words: `AgentPort`
//!    gives `agentport`, `agent` and `port`, and `agentport` gives `agentport`.
//! 4. **Words.** A word is lowercased, dropped if it is a stopword, and
//!    stemmed.
//!
//! The stopwords are two short lists written for this profile, an English one
//! and a French one, of articles, pronouns, prepositions, conjunctions and
//! common auxiliaries, looked up after folding (`à`, `où`, `été`). A text made
//! only of stopwords has no terms, and so an empty vector.
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
//! 2. **English endings.** `ed`, else `ing`, goes when a stem stays: `failed`
//!    and `failing` meet `fail`, and `retried` gives `retri`, but `need`,
//!    `tied` and `string` stay whole.
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
//! When steps 2 to 4 drop an ending, a final `bb`, `dd`, `ff`, `gg`, `mm`,
//! `nn`, `pp`, `rr` or `tt` then loses a letter if three letters stay: `logged`
//! and `logging` meet `log`, and `programme` meets `program`; but `installed`
//! keeps its `ll`, `planted` its `nt`, and `added` stays `add`.
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
