# Cut 8 mutations, H1-H39: each entry restores one permissiveness the cut
# closed and names the test that must fail while it is applied. Run through
# Epiphany's harness from this repo:
#
#   $env:CARGO_TARGET_DIR = 'C:\Users\Meta\.cargo-target-codex'
#   powershell -File F:\Projects\Epiphany\tools\eureka-mutations.ps1 -Repo F:\Projects\Huginn `
#       -Entries tools/eureka-cut8-mutations.psd1 `
#       -Target crates/huginn-mind/src/mind.rs,crates/huginn-mind/src/receipt.rs,crates/huginn-mind/src/admission.rs,crates/huginn-mind/src/store.rs `
#       -Test 'cargo test -p huginn-mind --lib'
#
# `store.rs` is a target only for H5, whose mutant needs a `MindStore` impl
# for the transient redb store that the live crate deliberately lacks.
@{
    Mutations = @(
        @{
            Id   = 'H1'
            Rule = 'Ruling 14: admission refuses another instance''s identity (A1).'
            Test = 'admission::tests::admission_refuses_a_foreign_instance_whatever_the_transport'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = '        if instance != self.instance() {'
            New  = '        if false {'
        }
        @{
            Id   = 'H2'
            Rule = 'Ruling 14: identity lives in the state; the opener compares the stored instance against the declared one.'
            Test = 'mind::tests::the_instance_document_is_the_identity_not_the_path'
            File = 'crates/huginn-mind/src/mind.rs'
            Old  = '    if stored.instance != *instance {'
            New  = '    if stored.instance != stored.instance {'
        }
        @{
            Id   = 'H3'
            Rule = 'Ruling 14: a mind is never un-owned; an empty mind''s first batch must carry the instance (A6).'
            Test = 'admission::tests::an_empty_store_opens_and_the_first_write_must_carry_the_instance'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = @'
        if self.is_empty() && !carries_instance {
            return Err(MindRefusal::MissingIdentity);
        }

'@
            New  = ''
        }
        @{
            Id   = 'H4'
            Rule = 'Ruling 14: the epoch record is derived on the first write (A6).'
            Test = 'admission::tests::an_empty_store_opens_and_the_first_write_must_carry_the_instance'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = @'
        if carries_instance {
            writes.push(HuginnMindEpoch::envelope(self.cache())?);
        }

'@
            New  = ''
        }
        @{
            Id    = 'H5'
            Rule  = 'Ruling 15: one writer; Mind::open chooses the owned store whose lock refuses a second owner.'
            Test  = 'mind::tests::a_mind_has_one_owner_at_a_time'
            Edits = @(
                @{
                    File = 'crates/huginn-mind/src/store.rs'
                    Old  = 'impl MindStore for OwnedRedbMessagePackBackingStore {'
                    New  = @'
impl MindStore for cultcache_rs::RedbMessagePackBackingStore {
    fn compare_and_swap_batch(
        &self,
        expected: &[CultCacheEnvelope],
        replacements: Vec<CultCacheEnvelope>,
    ) -> Result<bool> {
        cultcache_rs::RedbMessagePackBackingStore::compare_and_swap_batch(self, expected, replacements)
    }
}

impl MindStore for OwnedRedbMessagePackBackingStore {
'@
                }
                @{
                    File = 'crates/huginn-mind/src/mind.rs'
                    Old  = 'use cultcache_rs::{CultCache, CultCacheEnvelope, DatabaseEntry, OwnedRedbMessagePackBackingStore};'
                    New  = 'use cultcache_rs::{CultCache, CultCacheEnvelope, DatabaseEntry, OwnedRedbMessagePackBackingStore, RedbMessagePackBackingStore};'
                }
                @{
                    File = 'crates/huginn-mind/src/mind.rs'
                    Old  = 'impl Mind<OwnedRedbMessagePackBackingStore> {'
                    New  = 'impl Mind<RedbMessagePackBackingStore> {'
                }
                @{
                    File = 'crates/huginn-mind/src/mind.rs'
                    Old  = '        let store = OwnedRedbMessagePackBackingStore::new(&path).map_err(|error| {'
                    New  = '        let store = RedbMessagePackBackingStore::new(&path).map_err(|error| {'
                }
            )
        }
        @{
            Id   = 'H6'
            Rule = 'Ruling 20: the opener attaches nothing until every gate passes.'
            Test = 'mind::tests::the_opener_refuses_foreign_epoch_missing_identity_and_foreign_type_before_attaching'
            File = 'crates/huginn-mind/src/mind.rs'
            Old  = @'
        refuse_foreign_types(&raw)?;
        refuse_foreign_epoch(&raw)?;
        refuse_foreign_identity(&raw, instance)?;
        let (cache, image) = attach(store.clone())?;
'@
            New  = @'
        let (cache, image) = attach(store.clone())?;
        refuse_foreign_types(&raw)?;
        refuse_foreign_epoch(&raw)?;
        refuse_foreign_identity(&raw, instance)?;
'@
        }
        @{
            Id   = 'H7'
            Rule = 'Ruling 20: validation before replay; A9 runs after the per-kind rules, never before them.'
            Test = 'admission::tests::a_refused_batch_is_not_answered_from_a_stored_receipt'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = @'
        for staged in &docs.batch {
            check(&docs, staged, &mind)?;
        }
        // A9
        if let Some(existing) = receipt::replay(self, &candidate)? {
            return Ok(PipelineAdmissionOutcome::AlreadyAdmitted { receipt_id: existing.receipt_id });
        }
'@
            New  = @'
        if let Some(existing) = receipt::replay(self, &candidate)? {
            return Ok(PipelineAdmissionOutcome::AlreadyAdmitted { receipt_id: existing.receipt_id });
        }
        for staged in &docs.batch {
            check(&docs, staged, &mind)?;
        }
'@
        }
        @{
            Id   = 'H8'
            Rule = 'Q2/epoch: a store written at another epoch is refused.'
            Test = 'mind::tests::the_opener_refuses_foreign_epoch_missing_identity_and_foreign_type_before_attaching'
            File = 'crates/huginn-mind/src/mind.rs'
            Old  = '    if record.schema_epoch != PIPELINE_SCHEMA_EPOCH {'
            New  = '    if false {'
        }
        @{
            Id   = 'H9'
            Rule = 'Opener: a store holding a foreign type is refused before anything else.'
            Test = 'mind::tests::the_opener_refuses_foreign_epoch_missing_identity_and_foreign_type_before_attaching'
            File = 'crates/huginn-mind/src/mind.rs'
            Old  = @'
        refuse_foreign_types(&raw)?;

'@
            New  = ''
        }
        @{
            Id   = 'H10'
            Rule = 'Receipt: an exact replay answers with the stored receipt (A9).'
            Test = 'admission::tests::exact_replay_returns_already_admitted_across_provenance'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = '        if let Some(existing) = receipt::replay(self, &candidate)? {'
            New  = '        if let Some(existing) = receipt::replay(self, &candidate)?.filter(|_| false) {'
        }
        @{
            Id   = 'H11'
            Rule = 'Receipt: the digest is content, not author; provenance is never digested.'
            Test = 'admission::tests::a_receipt_names_the_exact_bytes_it_read_and_wrote'
            File = 'crates/huginn-mind/src/receipt.rs'
            Old  = '        let bytes = rmp_serde::to_vec_named(&(&self.instance, &self.strong_reads, &self.writes))'
            New  = '        let bytes = rmp_serde::to_vec_named(&(&self.instance, &self.strong_reads, &self.writes, &self.provenance))'
        }
        @{
            Id   = 'H12'
            Rule = 'Receipt: strong reads are the swap''s expectation, pinning the cited bytes (A11).'
            Test = 'admission::tests::a_receipt_names_the_exact_bytes_it_read_and_wrote'
            File = 'crates/huginn-mind/src/receipt.rs'
            Old  = '    let expected: &[CultCacheEnvelope] = &strong_reads;'
            New  = '    let expected: &[CultCacheEnvelope] = &[];'
        }
        @{
            Id   = 'H13'
            Rule = 'All-or-nothing: a lost swap is a Conflict, never a Committed (A11).'
            Test = 'admission::tests::batch_is_all_or_nothing'
            File = 'crates/huginn-mind/src/receipt.rs'
            Old  = '    if !landed {'
            New  = '    if false {'
        }
        @{
            Id   = 'H14'
            Rule = 'Supersession, not overwrite: a write colliding with the image is refused before the swap (A10).'
            Test = 'admission::tests::a_document_is_written_once_and_superseded_by_resolution'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = @'
        refuse_collisions(&docs)?;

'@
            New  = ''
        }
        @{
            Id   = 'H15'
            Rule = 'The resolution matrix is admission''s: every subject kind admits only its outcomes.'
            Test = 'admission::tests::the_resolution_matrix_is_admissions'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = @'
    fits
}
'@
            New  = @'
    true
}
'@
        }
        @{
            Id   = 'H16'
            Rule = '6c: a supersession names at least one supersessor.'
            Test = 'admission::tests::supersession_names_each_supersessor_and_none_is_empty'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = @'
    if let ResolutionOutcome::Superseded { by } = &resolution.outcome && by.is_empty() {
        return Err(MindRefusal::EmptySupersession);
    }

'@
            New  = ''
        }
        @{
            Id   = 'H17'
            Rule = 'Q10 + 6c: every supersessor and outcome referent must exist (A7).'
            Test = 'admission::tests::supersession_names_each_supersessor_and_none_is_empty'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = @'
            refs.extend(outcome_references(&resolution.outcome));

'@
            New  = ''
        }
        @{
            Id   = 'H18'
            Rule = '6c: an operator quote requires operator authority.'
            Test = 'admission::tests::an_operator_quote_requires_operator_authority'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = @'
            if ruling.operator_quote.is_some() && ruling.authority != RulingAuthority::Operator {
                return Err(MindRefusal::QuoteWithoutOperator { ruling: key.into() });
            }

'@
            New  = ''
        }
        @{
            Id   = 'H19'
            Rule = 'Ruling A: every promise of the cited report is measured by exactly one claim.'
            Test = 'admission::tests::verdict_vocabulary_binds_claims_to_findings_promises_and_mutations'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = '                if count != 1 {'
            New  = '                if count == 0 {'
        }
        @{
            Id   = 'H20'
            Rule = '6c: every mutation label a claim names exists in the report.'
            Test = 'admission::tests::verdict_vocabulary_binds_claims_to_findings_promises_and_mutations'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = @'
                for label in &claim.mutations {
                    if !report.mutations.iter().any(|mutation| mutation.label == *label) {
                        return Err(MindRefusal::UnknownMutationLabel { label: label.0.clone() });
                    }
                }

'@
            New  = ''
        }
        @{
            Id   = 'H21'
            Rule = 'Opener step 3: the epoch record is the record under the epoch''s own key; any other key is not it.'
            Test = 'mind::tests::the_opener_refuses_foreign_epoch_missing_identity_and_foreign_type_before_attaching'
            File = 'crates/huginn-mind/src/mind.rs'
            Old  = @'
    if record.key != PIPELINE_SCHEMA_EPOCH {
        return Err(foreign(record.key.clone()));
    }
'@
            New  = ''
        }
        @{
            Id   = 'H22'
            Rule = 'Opener step 3: exactly one epoch record, counted before any value is read, so the store''s order cannot decide the outcome.'
            Test = 'mind::tests::the_opener_refuses_foreign_epoch_missing_identity_and_foreign_type_before_attaching'
            File = 'crates/huginn-mind/src/mind.rs'
            Old  = @'
    let extra = records.count();
    if extra > 0 {
        return Err(foreign(format!("{} records", extra + 1)));
    }
'@
            New  = ''
        }
        @{
            Id   = 'H23'
            Rule = 'campaign: a campaign of no repos is refused, and by the organ''s own refusal (EmptyRepos), not a minted leaf bound.'
            Test = 'admission::tests::a_campaign_names_only_repos_this_mind_stewards'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = @'
            if campaign.repos.is_empty() {
                return Err(MindRefusal::EmptyRepos { campaign: key.into() });
            }
'@
            New  = ''
        }
        @{
            Id   = 'H24'
            Rule = 'A11: the receipt lands inside the batch''s own compare-and-swap, never in a second one after it.'
            Test = 'receipt::tests::a_commit_is_one_swap_carrying_the_documents_and_the_receipt_together'
            File = 'crates/huginn-mind/src/receipt.rs'
            Old  = @'
    replacements.push(receipt_envelope);
    let expected: &[CultCacheEnvelope] = &strong_reads;
    let landed = MindStore::compare_and_swap_batch(mind.store(), expected, replacements).map_err(unavailable)?;
'@
            New  = @'
    let expected: &[CultCacheEnvelope] = &strong_reads;
    let landed = MindStore::compare_and_swap_batch(mind.store(), expected, replacements).map_err(unavailable)?;
    let landed = landed && MindStore::compare_and_swap_batch(mind.store(), &[], vec![receipt_envelope]).map_err(unavailable)?;
'@
        }
        @{
            Id   = 'H25'
            Rule = 'A7: every reference the image resolved is pinned as a strong read, not the first of them.'
            Test = 'admission::tests::every_cited_image_document_is_a_strong_read'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = @'
            .filter_map(|(type_id, key)| self.raw_envelope(type_id, key).cloned())
            .collect::<Vec<_>>();
'@
            New  = @'
            .filter_map(|(type_id, key)| self.raw_envelope(type_id, key).cloned())
            .take(1)
            .collect::<Vec<_>>();
'@
        }
        @{
            Id   = 'H26'
            Rule = 'campaign: stewardship is looked for in image and batch, not in the batch alone.'
            Test = 'admission::tests::a_campaign_names_only_repos_this_mind_stewards'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = '                if docs.stewardship_of(mind, repo, None).is_none() {'
            New  = '                if !docs.batch.iter().any(|staged| matches!(&staged.document, D::Stewardship(stewardship) if stewardship.repo == *repo)) {'
        }
        @{
            Id   = 'H27'
            Rule = 'cut_report: the report''s repo must equal its spec''s, not only its branch.'
            Test = 'admission::tests::a_cut_report_cites_an_in_force_spec_and_agrees_with_it'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = @'
            if report.repo != spec.repo {
                return Err(MindRefusal::SpecMismatch { field: "repo".into() });
            }
'@
            New  = ''
        }
        @{
            Id   = 'H28'
            Rule = 'Ruling A: a promise is measured by the claim naming that label, not by any claim naming any promise.'
            Test = 'admission::tests::verdict_vocabulary_binds_claims_to_findings_promises_and_mutations'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = '                let count = verdict.claims.iter().filter(|claim| claim.promise.as_ref() == Some(&promise.label)).count();'
            New  = '                let count = verdict.claims.iter().filter(|claim| claim.promise.is_some()).count();'
        }
        @{
            Id   = 'H29'
            Rule = 'finding: the invariant vocabulary is the in-force target''s, not any target the campaign ever had.'
            Test = 'admission::tests::a_finding_names_evidence_locations_and_known_invariants'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = '                D::Target(target) if target.campaign == finding.campaign && docs.in_force(K::Target, target_key) => Some(target),'
            New  = '                D::Target(target) if target.campaign == finding.campaign && (docs.in_force(K::Target, target_key) || true) => Some(target),'
        }
        @{
            Id   = 'H30'
            Rule = 'The matrix is one row per kind: a question is answered or withdrawn, never superseded.'
            Test = 'admission::tests::the_resolution_matrix_is_admissions'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = @'
        (K::Question, O::Withdrawn { .. }) => true,
'@
            New  = @'
        (K::Question, O::Withdrawn { .. }) => true,
        (K::Question, O::Superseded { by }) => all(by, K::Question),
'@
        }
        @{
            Id   = 'H31'
            Rule = 'hand_off: the derived stewardship starts on the day the repo was handed over.'
            Test = 'admission::tests::a_hand_off_derives_this_minds_side_only'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = '                        assigned_on: hand_off.handed_on.clone(),'
            New  = '                        assigned_on: epiphany_pipeline::Date("2026-01-01".into()),'
        }
        @{
            Id   = 'H32'
            Rule = 'Derived writes pass A3-A7 like any other; a derived identity the batch already holds is A4''s refusal, not a store error.'
            Test = 'admission::tests::a_hand_off_derives_this_minds_side_only'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = @'
        for document in derive(&docs, &mind) {
            document.validate()?;
            let envelope = document.prepare(self.cache()).map_err(unavailable)?;
            docs.push(stage(envelope, &mind)?)?;
        }
        for staged in &docs.batch[originals..] {
            resolve(&docs, staged, &mind, &mut strong)?;
        }
'@
            New  = @'
        for document in derive(&docs, &mind) {
            let envelope = document.prepare(self.cache()).map_err(unavailable)?;
            docs.batch.push(Staged { kind: document.kind(), key: envelope.key.clone(), document, envelope });
        }
        let _ = originals;
'@
        }
        @{
            Id   = 'H33'
            Rule = 'A4: one identity, once, within a batch.'
            Test = 'admission::tests::a_batch_is_one_to_sixty_four_documents_each_with_its_own_identity'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = @'
        if self.batch.iter().any(|other| other.kind == staged.kind && other.key == staged.key) {
            return Err(MindRefusal::IdentityCollision { kind: staged.kind, id: staged.key });
        }
'@
            New  = ''
        }
        @{
            Id   = 'H34'
            Rule = 'A2: one to sixty-four envelopes, both ends checked.'
            Test = 'admission::tests::a_batch_is_one_to_sixty_four_documents_each_with_its_own_identity'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = '        if envelopes.is_empty() || envelopes.len() > BATCH_MAX {'
            New  = '        if false {'
        }
        @{
            Id   = 'H35'
            Rule = 'question: a question offers a real choice, at least two options, one of them the recommendation.'
            Test = 'admission::tests::a_question_offers_at_least_two_options_and_recommends_one_of_them'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = @'
            if question.options.len() < 2 || !question.options.iter().any(|option| option.label == question.recommended) {
                return Err(MindRefusal::InvalidOptions { question: key.into() });
            }
'@
            New  = ''
        }
        @{
            Id   = 'H36'
            Rule = 'question: an option label names one option.'
            Test = 'admission::tests::a_label_names_one_option_and_one_invariant'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = '            unique_labels("question.options", question.options.iter().map(|option| option.label.0.as_str()))'
            New  = '            Ok(())'
        }
        @{
            Id   = 'H37'
            Rule = 'target: an invariant label names one invariant.'
            Test = 'admission::tests::a_label_names_one_option_and_one_invariant'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = '            unique_labels("target.invariants", target.invariants.iter().map(|invariant| invariant.label.0.as_str()))'
            New  = '            Ok(())'
        }
        @{
            Id   = 'H38'
            Rule = 'finding: a finding names where in the code it lives, not only that evidence exists.'
            Test = 'admission::tests::a_finding_names_evidence_locations_and_known_invariants'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = '            if finding.evidence.is_empty() || finding.locations.is_empty() {'
            New  = '            if finding.evidence.is_empty() {'
        }
        @{
            Id   = 'H39'
            Rule = 'resolution: an Answered names the ruling that answers this subject, not some other question''s ruling.'
            Test = 'admission::tests::a_ruling_answering_a_question_derives_the_answered_resolution_atomically'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = @'
            if ruling.answers.as_ref().map(|answers| answers.0.as_str()) != Some(subject_id) {
                return Err(incompatible());
            }
'@
            New  = '            let _ = (ruling, subject_id);'
        }
    )
}
