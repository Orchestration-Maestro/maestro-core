//! The rules of `bm25-en-fr/1` as the lexical module states them, each with
//! an example it merges and one it must not: folding, identifiers, camelCase,
//! stopwords and the suffix rules, for French and English together.
#![cfg(test)]

use maestro_knowledge::lexical::terms;

/// No term at all.
const NONE: [&str; 0] = [];

#[test]
fn folding_drops_accents_in_either_case() {
    assert_eq!(terms("Élève ÉLÈVE élève eleve"), ["elev"; 4]);
    assert_eq!(
        terms("àâäçèéêëîïôöùûüÿ ÀÂÄÇÈÉÊËÎÏÔÖÙÛÜŸ"),
        ["aaaceeeeiioouuuy"; 2]
    );
}

#[test]
fn folding_spells_out_the_ligatures() {
    assert_eq!(terms("cœur Œuvre ÆTHER"), ["coeur", "oeuvr", "aeth"]);
}

#[test]
fn folding_drops_combining_marks_and_nothing_around_them() {
    assert_eq!(
        terms("re\u{300}sultat t\u{302}a\u{36f}che"),
        ["resultat", "tach"]
    );
    assert_eq!(terms("x\u{2ff}z \u{370}z"), ["x", "z", "\u{371}z"]);
}

#[test]
fn identifiers_give_their_whole_form_then_their_parts() {
    assert_eq!(
        terms("max_retries std::fs::read_to_string"),
        [
            "max_retries",
            "max",
            "retri",
            "std::fs::read_to_string",
            "std",
            "fs",
            "read",
            "string"
        ]
    );
    assert_eq!(terms("max retries"), ["max", "retri"]);
}

#[test]
fn a_backslash_joins_the_parts_of_a_windows_path() {
    assert_eq!(
        terms(r"D:\data\logs\app.log (D:\data\)"),
        [
            r"d:\data\logs\app.log",
            "data",
            "log",
            "app",
            "log",
            r"d:\data",
            "data"
        ]
    );
}

#[test]
fn a_word_over_64_characters_gives_only_its_whole_form() {
    let longest = "Abcdefgh".repeat(8);
    let mut parts = vec!["abcdefgh".repeat(8)];
    parts.extend(["abcdefgh"; 8].map(String::from));
    assert_eq!(terms(&longest), parts);
    assert_eq!(
        terms(&format!("{longest}X")),
        [format!("{}x", "abcdefgh".repeat(8))]
    );
    // Characters, not bytes: 64 Greek letters take 128 bytes.
    let greek = "Αβ".repeat(32);
    let mut parts = vec!["αβ".repeat(32)];
    parts.extend(["αβ"; 32].map(String::from));
    assert_eq!(terms(&greek), parts);
}

#[test]
fn punctuation_touching_an_identifier_is_trimmed() {
    assert_eq!(
        terms("(job-id) ERR-4012: --verbose"),
        ["job-id", "job", "id", "err-4012", "err", "4012", "verbos"]
    );
}

#[test]
fn apostrophes_separate_elided_words() {
    assert_eq!(
        terms("l'exécution L’erreur d'artefacts"),
        ["execution", "erreur", "artefact"]
    );
}

#[test]
fn camel_case_words_give_their_whole_form_then_their_parts() {
    assert_eq!(
        terms("AgentPort agentport base64Encoder HTTPServer"),
        [
            "agentport",
            "agent",
            "port",
            "agentport",
            "base64encoder",
            "base64",
            "encod",
            "httpserv",
            "http",
            "serv"
        ]
    );
}

#[test]
fn acronym_plurals_lose_their_s_before_lowercasing() {
    assert_eq!(terms("URLs APIs PDFs"), ["url", "api", "pdf"]);
    assert_eq!(
        terms("PDFs PDF CPUs CPU VMs VM SDKs SDK"),
        ["pdf", "pdf", "cpu", "cpu", "vm", "vm", "sdk", "sdk"]
    );
    assert_eq!(terms("Bus Gas Ms PDFS"), ["bus", "gas", "ms", "pdfs"]);
}

#[test]
fn plural_acronyms_and_short_capitals_stay_whole() {
    assert_eq!(
        terms("IDs OAuth ETag Œuvre"),
        ["id", "oauth", "etag", "oeuvr"]
    );
}

#[test]
fn stopwords_of_either_language_give_no_term() {
    assert_eq!(
        terms("the job of a lot, le lot de la tâche"),
        ["job", "lot", "lot", "tach"]
    );
    assert_eq!(terms("à où été être"), NONE);
}

#[test]
fn contracted_english_auxiliaries_are_stopwords() {
    assert_eq!(terms("don't isn't we'll you're they've"), NONE);
    assert_eq!(
        terms(
            "aren't couldn't didn't doesn't hadn't hasn't haven't mustn't shouldn't \
             wasn't weren't won't wouldn't"
        ),
        NONE
    );
}

#[test]
fn negation_words_are_stopwords_before_stemming() {
    assert_eq!(terms("not no ne n pas"), NONE);
    // `note` still stems to `not`: the adverb, a stopword, no longer adds to it.
    assert_eq!(
        terms("a note, not a notice; n'est pas une note"),
        ["not", "notic", "not"]
    );
}

#[test]
fn words_common_in_technical_text_are_not_stopwords() {
    assert_eq!(terms("AI CA AM"), ["ai", "ca", "am"]);
}

#[test]
fn words_holding_a_digit_are_not_stemmed() {
    assert_eq!(terms("utf8 ipv4s 4012"), ["utf8", "ipv4s", "4012"]);
}

#[test]
fn a_plural_s_meets_its_singular() {
    assert_eq!(
        terms("jobs job IDs id tâches tâche"),
        ["job", "job", "id", "id", "tach", "tach"]
    );
}

#[test]
fn y_counts_as_a_vowel_in_a_stem() {
    assert_eq!(
        terms("types type syncing sync"),
        ["typ", "typ", "sync", "sync"]
    );
}

#[test]
fn a_final_ss_us_or_lone_vowel_keeps_its_s() {
    assert_eq!(terms("class status gas"), ["class", "status", "gas"]);
}

#[test]
fn french_plurals_in_x_meet_their_singular() {
    assert_eq!(
        terms("réseaux réseau jeux jeu journaux journal"),
        ["reseau", "reseau", "jeu", "jeu", "journal", "journal"]
    );
}

#[test]
fn an_x_after_other_letters_is_kept() {
    assert_eq!(terms("index linux"), ["index", "linux"]);
}

#[test]
fn english_ed_and_ing_meet_the_bare_verb() {
    assert_eq!(
        terms("failed failing fail retried retries retry"),
        ["fail", "fail", "fail", "retri", "retri", "retri"]
    );
}

#[test]
fn ed_and_ing_are_kept_without_a_stem() {
    assert_eq!(terms("need string"), ["need", "string"]);
}

#[test]
fn ied_becomes_ie_so_short_verbs_meet_their_base() {
    assert_eq!(
        terms("tie ties tied die dies died"),
        ["tie", "tie", "tie", "die", "die", "die"]
    );
    assert_eq!(terms("cry cries cried"), ["cri", "cri", "cri"]);
}

#[test]
fn eed_verbs_meet_whatever_their_ending() {
    assert_eq!(
        terms("exceed exceeds exceeded exceeding"),
        ["exc", "exc", "exc", "exc"]
    );
    assert_eq!(
        terms("speed speeding bleed bleeding"),
        ["spe", "spe", "ble", "ble"]
    );
    assert_eq!(
        terms("agree agreed need needed recede receded"),
        ["agr", "agr", "need", "need", "reced", "reced"]
    );
}

#[test]
fn a_doubled_final_consonant_is_undoubled_once_an_ending_goes() {
    assert_eq!(
        terms("logged logging log programme program"),
        ["log", "log", "log", "program", "program"]
    );
}

#[test]
fn every_doubled_consonant_of_the_set_is_undoubled() {
    assert_eq!(
        terms(
            "grabbed grab padded pad tagged tag summed sum planned plan stopped stop \
             preferred prefer fitted fit"
        ),
        [
            "grab", "grab", "pad", "pad", "tag", "tag", "sum", "sum", "plan", "plan", "stop",
            "stop", "pref", "pref", "fit", "fit"
        ]
    );
}

#[test]
fn other_doubled_letters_stay_doubled() {
    assert_eq!(
        terms("stuffed stuff diffed diff passed pass buzzed buzz called call"),
        [
            "stuff", "stuff", "diff", "diff", "pass", "pass", "buzz", "buzz", "call", "call"
        ]
    );
}

#[test]
fn ll_different_letters_and_short_stems_keep_their_last_two() {
    assert_eq!(
        terms("installed install planted plant added add"),
        ["install", "install", "plant", "plant", "add", "add"]
    );
}

#[test]
fn french_participles_meet_whatever_their_gender_and_number() {
    assert_eq!(
        terms("planifiée planifiées planifié planifiés tâche"),
        ["planifi", "planifi", "planifi", "planifi", "tach"]
    );
}

#[test]
fn a_final_e_is_kept_without_a_stem() {
    assert_eq!(terms("one idée"), ["one", "ide"]);
}

#[test]
fn infinitives_meet_their_participles() {
    assert_eq!(
        terms("planifier planifié relancer relancés worker work"),
        ["planifi", "planifi", "relanc", "relanc", "work", "work"]
    );
}

#[test]
fn an_er_is_kept_without_a_stem() {
    assert_eq!(terms("hier user"), ["hier", "user"]);
}

#[test]
fn a_final_y_after_a_consonant_meets_its_ies_plural() {
    assert_eq!(terms("policy policies"), ["polici", "polici"]);
}

#[test]
fn a_final_y_after_a_vowel_is_kept() {
    assert_eq!(terms("day days key"), ["day", "day", "key"]);
}
