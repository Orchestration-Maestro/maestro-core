//! Raw SQL must not bypass frozen build authority or retained receipts.
use super::projection_schema_support::Fixture;

#[test]
fn projection_build_guards_admit_only_current_matching_attachment_and_resolution() {
    for replacement in [false, true] {
        let fixture = Fixture::new(replacement);
        fixture.migrate();
        let previous = replacement.then(|| fixture.generation());
        let job = fixture.job("admission");
        let valid = fixture.build_sql(&job, previous);
        let generation = fixture.generation().to_string();
        for (field, from, to) in [
            ("generation", format!("({generation},"), "(99999,".into()),
            ("collection", "'graph'".into(), "'other'".into()),
            (
                "claim set",
                fixture.receipt.identity.claim_set_id.as_str().into(),
                "0".repeat(64),
            ),
            ("job", format!("'{job}'"), "NULL".into()),
            (
                "format",
                "maestro-typed-edges/3".into(),
                "maestro-typed-edges/2".into(),
            ),
            (
                "resolution",
                fixture.receipt.resolution_id.as_str().into(),
                "0".repeat(64),
            ),
            (
                "resolver",
                fixture.receipt.resolver_version.clone(),
                "unknown/1".into(),
            ),
        ] {
            let invalid = valid.replacen(&from, &to, 1);
            assert_ne!(invalid, valid, "{field}: mutation did not apply");
            assert!(
                fixture.connection.execute_batch(&invalid).is_err(),
                "{field}"
            );
            assert_eq!(
                fixture.count("graph_projection_builds"),
                i64::from(replacement)
            );
        }
        fixture.insert_build(&job, previous).unwrap();
        fixture.foreign_keys_clean();
    }
}

#[test]
fn projection_build_guards_reject_each_invalid_pin_and_uncovered_snapshot() {
    let fixture = Fixture::new(false);
    fixture.migrate();
    let valid = fixture.build_sql(&fixture.job("pins"), None);
    for pin in [
        fixture.receipt.resolution_id.as_str(),
        fixture.receipt.settings_identity.as_str(),
        fixture.receipt.frozen_lock.as_str(),
        fixture.receipt.resolver_version.as_str(),
    ] {
        for invalid in [
            "NULL".to_owned(),
            "''".into(),
            "'bad'".into(),
            format!("'{}'", "G".repeat(64)),
        ] {
            assert!(
                fixture
                    .connection
                    .execute_batch(&valid.replace(&format!("'{pin}'"), &invalid))
                    .is_err(),
                "{pin}: {invalid}"
            );
        }
    }
    let resolution = fixture.receipt.resolution_id.as_str();
    fixture
        .connection
        .execute_batch("DROP TRIGGER graph_resolutions_immutable_update")
        .unwrap();
    for body in [
        "{\"sets\":[],\"resolver_version\":\"maestro-exact-resolution/1\"}",
        "{\"sets\":[],\"resolver_version\":\"unknown/1\"}",
    ] {
        fixture
            .connection
            .execute(
                "UPDATE graph_resolutions SET body = ?1 WHERE id = ?2",
                [body, resolution],
            )
            .unwrap();
        assert!(fixture.connection.execute_batch(&valid).is_err());
    }
    assert_eq!(fixture.count("graph_projection_builds"), 0);
}

#[test]
fn projection_build_guards_reject_stale_missing_and_cross_generation_heads() {
    for published in [false, true] {
        let fixture = Fixture::new(published);
        fixture.migrate();
        let job = fixture.job("head");
        let expected = published.then(|| fixture.generation());
        for wrong in [
            Some(99999),
            Some(fixture.generation() + 1),
            (!published).then_some(fixture.generation()),
        ] {
            assert!(fixture.insert_build(&job, wrong).is_err(), "head={wrong:?}");
        }
        fixture.insert_build(&job, expected).unwrap();
    }
}

#[test]
fn projection_build_guards_state_is_rechecked_at_reservation_and_receipt() {
    for existing in [false, true] {
        for state in ["building", "retired", "failed", "published"] {
            if existing && state == "published" {
                continue;
            }
            let fixture = Fixture::new(existing);
            fixture.migrate();
            let previous = existing.then(|| fixture.generation());
            let build = fixture.reserve(previous);
            fixture.set_state(state);
            assert!(
                fixture
                    .insert_build(&fixture.job("wrong-state"), previous)
                    .is_err(),
                "{state}"
            );
            assert!(
                fixture.insert_receipt(build, "candidate.db").is_err(),
                "{state}"
            );
            assert_eq!(
                fixture.count("graph_projection_receipts"),
                i64::from(existing)
            );
        }
    }
    let fixture = Fixture::new(true);
    fixture.migrate();
    fixture.set_state("published");
    let build = fixture.reserve(Some(fixture.generation()));
    fixture
        .insert_receipt(build, "published-repair.db")
        .unwrap();
    fixture.advance(build).unwrap();
}

#[test]
fn projection_build_guards_inputs_are_immutable_even_with_replace_and_recursive_triggers_off() {
    let fixture = Fixture::new(false);
    fixture.migrate();
    let job = fixture.job("immutable");
    fixture.insert_build(&job, None).unwrap();
    let build = fixture.connection.last_insert_rowid();
    let valid = fixture.build_sql(&job, None);
    assert!(
        fixture
            .connection
            .execute_batch(&valid.replace("INSERT INTO", "INSERT OR REPLACE INTO"))
            .is_err(),
        "job replacement"
    );
    let by_id = fixture
        .build_sql(&fixture.job("different-job"), None)
        .replace("(generation_id,", "(build_id, generation_id,")
        .replace("VALUES (", &format!("VALUES ({build}, "))
        .replace("INSERT INTO", "INSERT OR REPLACE INTO");
    assert!(
        fixture.connection.execute_batch(&by_id).is_err(),
        "id replacement"
    );
    for sql in [
        "UPDATE graph_projection_builds SET settings_identity = frozen_lock",
        "DELETE FROM graph_projection_builds",
    ] {
        assert!(fixture.connection.execute_batch(sql).is_err(), "{sql}");
    }
    assert_eq!(fixture.count("graph_projection_builds"), 1);
    fixture.foreign_keys_clean();
}

#[test]
fn projection_build_guards_receipts_counts_digest_names_and_retention() {
    let fixture = Fixture::new(false);
    fixture.migrate();
    let build = fixture.reserve(None);
    let insert = fixture.receipt_sql(build, "first.db");
    for (from, to) in [
        ("0, 0, 1", "1, 0, 1"),
        ("0, 0, 1", "0, -1, 1"),
        ("0, 0, 1", "0, 0, 0"),
        ("'first.db'", "'../outside.db'"),
        ("'first.db'", "'.leading.db'"),
        ("'first.db'", "''"),
    ] {
        assert!(
            fixture
                .connection
                .execute_batch(&insert.replace(from, to))
                .is_err(),
            "{from}: {to}"
        );
    }
    for digest in ["bad".to_owned(), "G".repeat(64), "z".repeat(64)] {
        assert!(
            fixture
                .connection
                .execute_batch(
                    &insert.replace(fixture.receipt.identity.content_digest.as_str(), &digest)
                )
                .is_err()
        );
    }
    fixture.insert_receipt(build, "first.db").unwrap();
    assert!(
        fixture
            .connection
            .execute_batch(&insert.replace("INSERT INTO", "INSERT OR REPLACE INTO"))
            .is_err(),
        "duplicate build"
    );
    let second = fixture.reserve(None);
    assert!(
        fixture
            .connection
            .execute_batch(
                &fixture
                    .receipt_sql(second, "first.db")
                    .replace("INSERT INTO", "INSERT OR REPLACE INTO")
            )
            .is_err(),
        "duplicate filename"
    );
    for sql in [
        "UPDATE graph_projection_receipts SET file_name = 'other.db'",
        "DELETE FROM graph_projection_receipts",
    ] {
        assert!(fixture.connection.execute_batch(sql).is_err(), "{sql}");
    }
    assert_eq!(fixture.count("graph_projection_receipts"), 1);
    fixture.foreign_keys_clean();
}

#[test]
fn projection_build_guards_stale_candidate_cannot_receipt_after_a_head_change() {
    let fixture = Fixture::new(true);
    fixture.migrate();
    let first = fixture.reserve(Some(fixture.generation()));
    let stale = fixture.reserve(Some(fixture.generation()));
    fixture.insert_receipt(first, "winner.db").unwrap();
    fixture.advance(first).unwrap();
    assert!(fixture.insert_receipt(stale, "stale.db").is_err());
    assert!(
        fixture
            .insert_build(&fixture.job("stale"), Some(fixture.generation()))
            .is_err()
    );
    assert_eq!(fixture.count("graph_projection_receipts"), 2);
    assert_eq!(fixture.head(), first);
}

#[test]
fn projection_build_guards_pinned_predecessor_keeps_resolution_but_new_settings_are_allowed() {
    let fixture = Fixture::new(true);
    fixture.migrate();
    let alternate = fixture.alternate_resolution();
    let valid = fixture.build_sql(&fixture.job("new-pins"), Some(fixture.generation()));
    assert!(
        fixture
            .connection
            .execute_batch(&valid.replace(fixture.receipt.resolution_id.as_str(), &alternate))
            .is_err()
    );
    let changed = valid
        .replace(fixture.receipt.settings_identity.as_str(), &"1".repeat(64))
        .replace(fixture.receipt.frozen_lock.as_str(), &"2".repeat(64));
    fixture.connection.execute_batch(&changed).unwrap();
    fixture.foreign_keys_clean();
}

#[test]
fn projection_build_guards_publication_rechecks_resolution_and_nonlegacy_build() {
    let fixture = Fixture::new(true);
    fixture.migrate();
    assert!(
        fixture
            .insert_receipt(fixture.generation(), "legacy-again.db")
            .is_err()
    );
    let build = fixture.reserve(Some(fixture.generation()));
    fixture
        .connection
        .execute_batch(
            "DROP TRIGGER graph_resolutions_immutable_update;
        UPDATE graph_resolutions SET body = '{\"sets\":[],
            \"resolver_version\":\"maestro-exact-resolution/1\"}';",
        )
        .unwrap();
    assert!(fixture.insert_receipt(build, "uncovered.db").is_err());
    assert_eq!(fixture.count("graph_projection_receipts"), 1);
}

#[test]
fn projection_build_guards_reject_nonpositive_ids_and_noncanonical_covering_resolution() {
    let fixture = Fixture::new(false);
    fixture.migrate();
    let valid = fixture.build_sql(&fixture.job("canonical"), None);
    for id in [0, -1] {
        let invalid = valid
            .replace("(generation_id,", "(build_id, generation_id,")
            .replace("VALUES (", &format!("VALUES ({id}, "));
        assert!(fixture.connection.execute_batch(&invalid).is_err());
    }
    let uppercase = "A".repeat(64);
    fixture
        .connection
        .execute(
            "INSERT INTO graph_resolutions (id, reviewer, body)
        SELECT ?1, reviewer, body FROM graph_resolutions WHERE id = ?2",
            [&uppercase, fixture.receipt.resolution_id.as_str()],
        )
        .unwrap();
    assert!(
        fixture
            .connection
            .execute_batch(&valid.replace(fixture.receipt.resolution_id.as_str(), &uppercase))
            .is_err()
    );
    fixture.connection.execute_batch(&valid).unwrap();
    fixture.foreign_keys_clean();
}

#[test]
fn projection_build_guards_admission_checks_resolver_body_with_coverage_intact() {
    let fixture = Fixture::new(false);
    fixture.migrate();
    let job = fixture.job("resolver-body");
    fixture
        .connection
        .execute_batch("DROP TRIGGER graph_resolutions_immutable_update")
        .unwrap();
    fixture
        .connection
        .execute(
            "UPDATE graph_resolutions SET body = json_set(body, '$.resolver_version', 'other/1')
         WHERE id = ?1",
            [fixture.receipt.resolution_id.as_str()],
        )
        .unwrap();
    assert!(fixture.insert_build(&job, None).is_err());
    assert_eq!(fixture.count("graph_projection_builds"), 0);
}

#[test]
fn projection_build_guards_publication_checks_resolver_only_body_change() {
    let fixture = Fixture::new(false);
    fixture.migrate();
    let build = fixture.reserve(None);
    fixture
        .connection
        .execute_batch("DROP TRIGGER graph_resolutions_immutable_update")
        .unwrap();
    fixture
        .connection
        .execute(
            "UPDATE graph_resolutions SET body = json_set(body, '$.resolver_version', 'other/1')
         WHERE id = ?1",
            [fixture.receipt.resolution_id.as_str()],
        )
        .unwrap();
    assert!(fixture.insert_receipt(build, "candidate.db").is_err());
    assert_eq!(fixture.count("graph_projection_receipts"), 0);
    assert_eq!(fixture.count("graph_projection_active"), 0);
}
