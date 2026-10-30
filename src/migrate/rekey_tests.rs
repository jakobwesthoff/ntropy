// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Tests of the rekey's key handling (ADR 0080).
//!
//! Each test stops a rekey at a point a crash could, through the migration
//! hooks, then looks at what is on disk and resumes.

use std::os::unix::fs::PermissionsExt;

use super::*;
use crate::migrate::{MigrationHooks, NoHooks, Phase};
use crate::test_support::encrypted::{
    EncryptedVault, encrypted_vault_with_wrapped_identity, write_encrypted_note,
};

const PASSPHRASE: &str = "correct horse";
const ULID_A: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const ULID_B: &str = "01BX5ZZKBKACTAV9WEVGEMMVRZ";
const ALPHA: &str = "---\ntitle: Alpha\n---\nfirst body\n";
const BETA: &str = "---\ntitle: Beta\n---\nsecond body\n";

/// Where a test stops the rekey.
#[derive(Clone, Copy)]
enum Stop {
    Verify,
    Step(usize),
    Commit,
    Keys,
}

struct StopAt(Stop);

impl StopAt {
    fn when(&self, here: bool) -> Result<(), MigrateError> {
        match here {
            true => Err(MigrateError::Interrupted(Phase::Commit)),
            false => Ok(()),
        }
    }
}

impl MigrationHooks for StopAt {
    fn after_verify(&self) -> Result<(), MigrateError> {
        self.when(matches!(self.0, Stop::Verify))
    }
    fn after_commit_step(&self, replaced: usize) -> Result<(), MigrateError> {
        self.when(matches!(self.0, Stop::Step(n) if n == replaced))
    }
    fn after_commit(&self) -> Result<(), MigrateError> {
        self.when(matches!(self.0, Stop::Commit))
    }
    fn between_key_files(&self) -> Result<(), MigrateError> {
        self.when(matches!(self.0, Stop::Keys))
    }
}

/// Replaces the staged recipient while the rekey runs, the way a second
/// process or a sync tool could.
struct Tamper(PathBuf);

impl MigrationHooks for Tamper {
    fn after_verify(&self) -> Result<(), MigrateError> {
        let (_, other) = age_io::generate_keypair();
        std::fs::write(&self.0, format!("{}\n", other.as_str())).expect("tamper");
        Ok(())
    }
}

fn vault() -> EncryptedVault {
    let vault = encrypted_vault_with_wrapped_identity(PASSPHRASE);
    write_encrypted_note(&vault.session, ULID_A, ALPHA);
    write_encrypted_note(&vault.session, ULID_B, BETA);
    vault
}

fn rekey_as(
    vault: &EncryptedVault,
    resume: bool,
    passphrase: &str,
    hooks: &dyn MigrationHooks,
) -> Result<RekeyOutcome, MigrateError> {
    rekey(&Rekey {
        session: &vault.session,
        source: vault.session.cipher(),
        source_recipient: &vault.identity.to_public(),
        passphrase: &Passphrase::new(passphrase),
        resume,
        hooks,
        started: "2026-10-30T12:00:00Z".into(),
    })
}

fn run_rekey(
    vault: &EncryptedVault,
    resume: bool,
    hooks: &dyn MigrationHooks,
) -> Result<RekeyOutcome, MigrateError> {
    rekey_as(vault, resume, PASSPHRASE, hooks)
}

fn layout(vault: &EncryptedVault) -> &Layout {
    vault.session.layout()
}

fn note(vault: &EncryptedVault, ulid: &str) -> PathBuf {
    layout(vault).all_notes().join(format!("{ulid}.age"))
}

fn marker_of(vault: &EncryptedVault) -> Marker {
    marker::read_at(&layout(vault).migration_file())
        .expect("read marker")
        .expect("a marker is present")
}

fn opens(identity: &Identity, path: &Path) -> Option<String> {
    AgeCipher::new(identity.to_public(), Some(identity.clone()))
        .read(path)
        .ok()
}

fn staged_identity(vault: &EncryptedVault) -> Identity {
    keys::unwrap_identity_at(
        &layout(vault).identity_file_staged(),
        &Passphrase::new(PASSPHRASE),
    )
    .expect("the staged identity opens")
}

fn live_identity(vault: &EncryptedVault) -> Identity {
    keys::unwrap_identity_at(&layout(vault).identity_file(), &Passphrase::new(PASSPHRASE))
        .expect("the live identity opens")
}

/// Every note opens with the identity wrapped in `.ntropy/` right now or with
/// the staged one.
fn assert_no_note_is_stranded(vault: &EncryptedVault) {
    let mut identities = vec![live_identity(vault)];
    if layout(vault).identity_file_staged().exists() {
        identities.push(staged_identity(vault));
    }
    for ulid in [ULID_A, ULID_B] {
        assert!(
            identities
                .iter()
                .any(|identity| opens(identity, &note(vault, ulid)).is_some()),
            "{ulid} opens with no key on disk"
        );
    }
}

/// The vault ends with exactly one keypair, the one named `recipient`, and
/// every note opens with it.
fn assert_finished_with(vault: &EncryptedVault, recipient: &Recipient) {
    let layout = layout(vault);
    assert_eq!(
        keys::read_recipient_at(&layout.identity_pub()).expect("read"),
        Some(recipient.clone())
    );
    let identity = live_identity(vault);
    assert_eq!(&identity.to_public(), recipient);
    assert_eq!(
        opens(&identity, &note(vault, ULID_A)).as_deref(),
        Some(ALPHA)
    );
    assert_eq!(
        opens(&identity, &note(vault, ULID_B)).as_deref(),
        Some(BETA)
    );
    assert!(!layout.identity_pub_staged().exists());
    assert!(!layout.identity_file_staged().exists());
    assert!(!layout.migration_file().exists());
}

#[test]
fn the_new_key_is_on_disk_before_any_source_is_replaced() {
    let vault = vault();
    run_rekey(&vault, false, &StopAt(Stop::Verify)).expect_err("stopped");

    let staged = staged_identity(&vault);
    let marker = marker_of(&vault);
    assert_eq!(marker.recipient, Some(staged.to_public().as_str()));
    assert!(marker.staged && !marker.handover);
    assert_eq!(
        marker.source_recipient,
        Some(vault.identity.to_public().as_str())
    );
    let mode = std::fs::metadata(layout(&vault).identity_file_staged())
        .expect("metadata")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);

    // The vault's own identity is still the old one.
    assert_eq!(
        live_identity(&vault).to_public(),
        vault.identity.to_public()
    );
    assert_no_note_is_stranded(&vault);
}

#[test]
fn a_resume_installs_the_key_that_was_staged() {
    let vault = vault();
    run_rekey(&vault, false, &StopAt(Stop::Verify)).expect_err("stopped");
    let staged = staged_identity(&vault).to_public();

    let outcome = run_rekey(&vault, true, &NoHooks).expect("resume");

    assert_eq!(outcome.recipient, staged);
    assert_eq!(outcome.report.deleted, 2);
    assert_finished_with(&vault, &staged);
}

#[test]
fn an_uninterrupted_rekey_ends_on_the_new_key_alone() {
    let vault = vault();
    let outcome = run_rekey(&vault, false, &NoHooks).expect("rekey");

    assert_ne!(outcome.recipient, vault.identity.to_public());
    assert_finished_with(&vault, &outcome.recipient);
}

#[test]
fn an_empty_vault_gets_its_new_key() {
    let vault = encrypted_vault_with_wrapped_identity(PASSPHRASE);
    let outcome = run_rekey(&vault, false, &NoHooks).expect("rekey");

    assert_eq!(outcome.report.deleted, 0);
    assert_eq!(live_identity(&vault).to_public(), outcome.recipient);
}

#[test]
fn a_stop_after_the_first_replacement_leaves_every_note_readable_and_resumes() {
    let vault = vault();
    run_rekey(&vault, false, &StopAt(Stop::Step(1))).expect_err("stopped");

    assert_no_note_is_stranded(&vault);
    let staged = staged_identity(&vault);
    let old = live_identity(&vault);
    let with_new = [ULID_A, ULID_B]
        .into_iter()
        .filter(|ulid| opens(&staged, &note(&vault, ulid)).is_some())
        .count();
    let with_old = [ULID_A, ULID_B]
        .into_iter()
        .filter(|ulid| opens(&old, &note(&vault, ulid)).is_some())
        .count();
    assert_eq!((with_new, with_old), (1, 1));

    run_rekey(&vault, true, &NoHooks).expect("resume");
    assert_finished_with(&vault, &staged.to_public());
}

#[test]
fn a_stop_after_every_replacement_resumes_into_the_handover() {
    let vault = vault();
    run_rekey(&vault, false, &StopAt(Stop::Commit)).expect_err("stopped");

    assert_no_note_is_stranded(&vault);
    let staged = staged_identity(&vault);
    for ulid in [ULID_A, ULID_B] {
        assert!(opens(&staged, &note(&vault, ulid)).is_some());
    }
    assert!(!marker_of(&vault).handover);
    assert_eq!(
        live_identity(&vault).to_public(),
        vault.identity.to_public()
    );

    run_rekey(&vault, true, &NoHooks).expect("resume");
    assert_finished_with(&vault, &staged.to_public());
}

#[test]
fn a_stop_between_the_two_identity_files_leaves_the_wrapped_one_opening_the_notes() {
    let vault = vault();
    run_rekey(&vault, false, &StopAt(Stop::Keys)).expect_err("stopped");
    let layout = layout(&vault);

    // The wrapped identity already holds the new key, the recipient file still
    // names the old one, and the staged pair is intact.
    let installed = live_identity(&vault);
    assert_eq!(installed.to_public(), staged_identity(&vault).to_public());
    assert_eq!(
        keys::read_recipient_at(&layout.identity_pub()).expect("read"),
        Some(vault.identity.to_public())
    );
    assert!(layout.identity_pub_staged().exists());
    for ulid in [ULID_A, ULID_B] {
        assert!(opens(&installed, &note(&vault, ulid)).is_some());
    }
    assert!(marker_of(&vault).handover);
}

#[test]
fn a_resume_after_the_stop_between_the_identity_files_finishes_without_keys() {
    let vault = vault();
    run_rekey(&vault, false, &StopAt(Stop::Keys)).expect_err("stopped");
    let installed = live_identity(&vault).to_public();

    // Nothing here needs the old key or the right passphrase.
    let outcome = rekey_as(&vault, true, "no such passphrase", &NoHooks).expect("resume");

    assert_eq!(outcome.report.deleted, 0);
    assert_finished_with(&vault, &installed);
}

#[test]
fn finishing_the_handover_reports_the_key_it_retired() {
    let vault = vault();
    run_rekey(&vault, false, &StopAt(Stop::Keys)).expect_err("stopped");

    let retired = finish_handover(layout(&vault), &NoHooks).expect("finish");

    assert_eq!(retired, Some(vault.identity.to_public()));
    assert!(!layout(&vault).migration_file().exists());
    assert!(matches!(
        finish_handover(layout(&vault), &NoHooks),
        Err(MigrateError::NothingToResume)
    ));
}

#[test]
fn a_handover_that_stopped_after_copying_both_files_runs_again() {
    let vault = vault();
    run_rekey(&vault, false, &StopAt(Stop::Keys)).expect_err("stopped");
    // The crash comes after the recipient was copied too, with both staged
    // files still in place.
    let layout = layout(&vault);
    std::fs::copy(layout.identity_pub_staged(), layout.identity_pub()).expect("copy");

    finish_handover(layout, &NoHooks).expect("finish");

    assert_finished_with(&vault, &live_identity(&vault).to_public());
}

#[test]
fn a_handover_that_stopped_with_only_the_staged_recipient_left_runs_again() {
    let vault = vault();
    run_rekey(&vault, false, &StopAt(Stop::Keys)).expect_err("stopped");
    let layout = layout(&vault);
    std::fs::remove_file(layout.identity_file_staged()).expect("remove");

    finish_handover(layout, &NoHooks).expect("finish");

    assert_finished_with(&vault, &live_identity(&vault).to_public());
}

#[test]
fn a_note_whose_source_went_before_its_replacement_took_the_name_is_adopted() {
    let vault = vault();
    run_rekey(&vault, false, &StopAt(Stop::Verify)).expect_err("stopped");
    std::fs::remove_file(note(&vault, ULID_A)).expect("remove source");

    run_rekey(&vault, true, &NoHooks).expect("resume");

    assert_finished_with(&vault, &live_identity(&vault).to_public());
}

#[test]
fn an_orphaned_file_no_key_opens_is_not_taken_for_the_note() {
    let vault = vault();
    run_rekey(&vault, false, &StopAt(Stop::Verify)).expect_err("stopped");
    let orphan = PathBuf::from(format!("{}{REKEY_SUFFIX}", note(&vault, ULID_A).display()));
    std::fs::remove_file(note(&vault, ULID_A)).expect("remove source");
    std::fs::write(&orphan, "not a ciphertext").expect("overwrite");

    let error = run_rekey(&vault, true, &NoHooks).expect_err("refused");

    assert!(
        matches!(&error, MigrateError::UnreadableNotes(paths) if paths == &[orphan]),
        "{error:?}"
    );
}

#[test]
fn a_staged_recipient_that_changed_before_the_commit_costs_nothing() {
    let vault = vault();
    let tamper = Tamper(layout(&vault).identity_pub_staged());

    let error = run_rekey(&vault, false, &tamper).expect_err("refused");

    assert!(matches!(error, MigrateError::StagedKeyMismatch { .. }));
    // Every source is still in place and opens with the old key.
    for ulid in [ULID_A, ULID_B] {
        assert!(
            opens(&vault.identity, &note(&vault, ulid)).is_some(),
            "{ulid}"
        );
    }
}

#[test]
fn a_staged_recipient_that_changed_since_the_marker_stops_the_resume() {
    let vault = vault();
    run_rekey(&vault, false, &StopAt(Stop::Verify)).expect_err("stopped");
    let (_, other) = age_io::generate_keypair();
    std::fs::write(
        layout(&vault).identity_pub_staged(),
        format!("{}\n", other.as_str()),
    )
    .expect("tamper");

    let error = run_rekey(&vault, true, &NoHooks).expect_err("refused");

    assert!(matches!(error, MigrateError::StagedKeyMismatch { .. }));
}

#[test]
fn a_staged_wrapped_identity_of_another_key_stops_the_resume() {
    let vault = vault();
    run_rekey(&vault, false, &StopAt(Stop::Verify)).expect_err("stopped");
    let (other, _) = age_io::generate_keypair();
    std::fs::write(
        layout(&vault).identity_file_staged(),
        age_io::wrap_identity(&other, &Passphrase::new(PASSPHRASE)).expect("wrap"),
    )
    .expect("tamper");

    let error = run_rekey(&vault, true, &NoHooks).expect_err("refused");

    assert!(matches!(error, MigrateError::StagedKeyMismatch { .. }));
}

#[test]
fn a_missing_staged_pair_is_named_and_not_replaced() {
    let vault = vault();
    run_rekey(&vault, false, &StopAt(Stop::Step(1))).expect_err("stopped");
    std::fs::remove_file(layout(&vault).identity_file_staged()).expect("remove");

    let error = run_rekey(&vault, true, &NoHooks).expect_err("refused");

    assert!(matches!(&error, MigrateError::StagedKeyMissing(paths) if paths.len() == 1));
    assert!(marker_of(&vault).staged);
}

#[test]
fn the_wrong_passphrase_for_the_staged_key_is_reported_as_that() {
    let vault = vault();
    run_rekey(&vault, false, &StopAt(Stop::Verify)).expect_err("stopped");

    let error = rekey_as(&vault, true, "not the one", &NoHooks).expect_err("refused");

    assert!(matches!(error, MigrateError::StagedKeyLocked { .. }));
    assert!(error.to_string().contains("--new-passphrase-file"));
}

#[test]
fn a_second_start_leaves_the_interrupted_rekeys_staged_key_alone() {
    let vault = vault();
    run_rekey(&vault, false, &StopAt(Stop::Step(1))).expect_err("stopped");
    let staged = std::fs::read(layout(&vault).identity_file_staged()).expect("read");

    let error = run_rekey(&vault, false, &NoHooks).expect_err("refused");

    assert!(matches!(
        error,
        MigrateError::AlreadyInProgress {
            operation: Operation::Rekey,
            ..
        }
    ));
    assert_eq!(
        std::fs::read(layout(&vault).identity_file_staged()).expect("read"),
        staged
    );
}

#[test]
fn a_resume_without_an_interrupted_rekey_is_refused() {
    let vault = vault();
    assert!(matches!(
        run_rekey(&vault, true, &NoHooks),
        Err(MigrateError::NothingToResume)
    ));
}

#[test]
fn a_resume_does_not_finish_another_operations_marker() {
    let vault = vault();
    let encrypting = Marker::new(Operation::Encrypt, Some("age1fake".into()), "x".into());
    marker::write_at(&layout(&vault).migration_file(), &encrypting).expect("marker");

    let error = run_rekey(&vault, true, &NoHooks).expect_err("refused");

    assert!(matches!(
        error,
        MigrateError::AlreadyInProgress {
            operation: Operation::Encrypt,
            ..
        }
    ));
    assert_eq!(marker_of(&vault), encrypting);
}

#[test]
fn a_rekey_marker_without_a_target_key_is_refused() {
    let vault = vault();
    let mut broken = Marker::new(Operation::Rekey, None, "x".into());
    broken.staged = true;
    marker::write_at(&layout(&vault).migration_file(), &broken).expect("marker");

    assert!(matches!(
        run_rekey(&vault, true, &NoHooks),
        Err(MigrateError::MarkerWithoutRecipient)
    ));
}

#[test]
fn a_note_the_old_key_cannot_open_stops_a_rekey_before_it_starts() {
    let vault = vault();
    std::fs::write(note(&vault, ULID_B), "not a ciphertext").expect("damage");

    let error = run_rekey(&vault, false, &NoHooks).expect_err("refused");

    assert!(matches!(&error, MigrateError::UnreadableNotes(paths) if paths.len() == 1));
    assert!(!layout(&vault).identity_pub_staged().exists());
    assert!(!layout(&vault).migration_file().exists());
}

#[test]
fn a_file_in_the_way_of_a_produced_note_stops_a_rekey_before_a_key_is_staged() {
    let vault = vault();
    let in_the_way = layout(&vault)
        .all_notes()
        .join(format!("{ULID_A}.age{REKEY_SUFFIX}"));
    std::fs::write(&in_the_way, "a stray file").expect("stray file");

    let error = run_rekey(&vault, false, &NoHooks).expect_err("refused");

    assert!(
        matches!(error, MigrateError::TargetsExist { .. }),
        "{error:?}"
    );
    assert!(!layout(&vault).identity_pub_staged().exists());
}

#[test]
fn a_leftover_staged_pair_is_replaced_by_the_next_start() {
    let vault = vault();
    let layout = layout(&vault);
    let (leftover, _) = age_io::generate_keypair();
    keys::stage_keypair_at(
        &layout.identity_pub_staged(),
        &layout.identity_file_staged(),
        &leftover,
        &Passphrase::new("some other passphrase"),
    )
    .expect("stage");

    let outcome = run_rekey(&vault, false, &NoHooks).expect("rekey");

    assert_ne!(outcome.recipient, leftover.to_public());
    assert_finished_with(&vault, &outcome.recipient);
}

#[test]
fn a_leftover_staged_pair_beside_an_orphaned_file_is_kept() {
    let vault = vault();
    let layout = layout(&vault);
    let (leftover, _) = age_io::generate_keypair();
    keys::stage_keypair_at(
        &layout.identity_pub_staged(),
        &layout.identity_file_staged(),
        &leftover,
        &Passphrase::new(PASSPHRASE),
    )
    .expect("stage");
    let orphan = layout
        .all_notes()
        .join(format!("{ULID_B}.age{REKEY_SUFFIX}"));
    std::fs::rename(note(&vault, ULID_B), &orphan).expect("strand");

    let error = run_rekey(&vault, false, &NoHooks).expect_err("refused");

    assert!(matches!(&error, MigrateError::StrandedNotes(paths) if paths == &[orphan]));
    assert_eq!(
        staged_identity(&vault).to_public(),
        leftover.to_public(),
        "the staged key may be the only one that opens that file"
    );
}

/// The state an interrupted rekey of a version that kept its new key in
/// memory leaves: the marker without the `staged` flag and no staged pair.
fn forget_the_staged_key(vault: &EncryptedVault) {
    let layout = layout(vault);
    std::fs::remove_file(layout.identity_pub_staged()).expect("remove");
    std::fs::remove_file(layout.identity_file_staged()).expect("remove");
    let mut marker = marker_of(vault);
    marker.staged = false;
    marker::write_at(&layout.migration_file(), &marker).expect("marker");
}

#[test]
fn a_rekey_that_never_staged_its_key_starts_over_when_nothing_was_replaced() {
    let vault = vault();
    run_rekey(&vault, false, &StopAt(Stop::Verify)).expect_err("stopped");
    let before = marker_of(&vault);
    forget_the_staged_key(&vault);

    let outcome = run_rekey(&vault, true, &NoHooks).expect("resume");

    assert_ne!(Some(outcome.recipient.as_str()), before.recipient);
    assert_finished_with(&vault, &outcome.recipient);
}

#[test]
fn a_rekey_that_never_staged_its_key_names_the_notes_it_lost() {
    let vault = vault();
    run_rekey(&vault, false, &StopAt(Stop::Step(1))).expect_err("stopped");
    forget_the_staged_key(&vault);

    let error = run_rekey(&vault, true, &NoHooks).expect_err("refused");

    assert!(matches!(&error, MigrateError::StrandedNotes(paths) if paths.len() == 1));
    assert!(!layout(&vault).identity_pub_staged().exists());
}

#[test]
fn a_rekey_that_never_staged_its_key_names_an_orphaned_file() {
    let vault = vault();
    run_rekey(&vault, false, &StopAt(Stop::Verify)).expect_err("stopped");
    forget_the_staged_key(&vault);
    std::fs::remove_file(note(&vault, ULID_A)).expect("remove source");

    let error = run_rekey(&vault, true, &NoHooks).expect_err("refused");

    assert!(matches!(&error, MigrateError::StrandedNotes(paths) if paths.len() == 1));
}

#[test]
fn a_note_neither_key_opens_stops_the_handover_before_the_old_key_goes() {
    let vault = vault();
    run_rekey(&vault, false, &StopAt(Stop::Commit)).expect_err("stopped");
    std::fs::write(note(&vault, ULID_A), "not a ciphertext").expect("damage");

    let error = run_rekey(&vault, true, &NoHooks).expect_err("refused");

    assert!(matches!(&error, MigrateError::UnreadableNotes(paths) if paths.len() == 1));
    assert_eq!(
        live_identity(&vault).to_public(),
        vault.identity.to_public()
    );
    assert!(!marker_of(&vault).handover);
}

#[test]
fn a_handover_refuses_a_staged_recipient_other_than_the_markers() {
    let vault = vault();
    run_rekey(&vault, false, &StopAt(Stop::Keys)).expect_err("stopped");
    let (_, other) = age_io::generate_keypair();
    std::fs::write(
        layout(&vault).identity_pub_staged(),
        format!("{}\n", other.as_str()),
    )
    .expect("tamper");

    assert!(matches!(
        finish_handover(layout(&vault), &NoHooks),
        Err(MigrateError::StagedKeyMismatch { .. })
    ));
}

#[test]
fn a_missing_staged_recipient_fails_the_check() {
    let vault = vault();
    let handover = KeyHandover::new(layout(&vault), &vault.identity.to_public());

    assert!(matches!(
        handover.check_staged(),
        Err(MigrateError::StagedKeyMissing(_))
    ));
}
