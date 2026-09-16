# Cut 8 mutations, H1-H20: each entry restores one permissiveness the cut
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
    )
}
