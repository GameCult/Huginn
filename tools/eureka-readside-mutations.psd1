# RS-1 mutations, R1-R5: the receipt ordinal, receipt v2, and the epoch-first
# opener. Each rule has a revert, `Rn`, and (where a distinct weaker shape
# exists) a loosening, `RnL`. Every entry names the test that must fail while
# it is applied. Run through the Eureka skill's harness from this repo:
#
#   $env:CARGO_TARGET_DIR = 'C:\Users\Meta\.cargo-target-codex'
#   powershell -File C:\Users\Meta\.claude\skills\eureka\tools\eureka-mutations.ps1 -Repo F:\Projects\Huginn `
#       -Entries tools/eureka-readside-mutations.psd1 `
#       -Target crates/huginn-mind/src/receipt.rs,crates/huginn-mind/src/admission.rs,crates/huginn-mind/src/mind.rs `
#       -Test 'cargo test -p huginn-mind --lib'
#
# `M0` is the harness's own no-op control and is not listed here.
#
# R2 has no loosening and says so rather than carrying a weak entry: "the
# ordinal is not digested" has one natural mutant, putting it back in the
# digest. There is no partial shape of "digested a little" to weaken it into.
#
# R5 has no loosening for the same reason D9 and D12 in cut10 give: the rule
# is an order between two calls, and the only two orders are the order and
# its reverse. Swapping the gates back is the revert and the whole space of
# wrong orders at once.
@{
    Mutations = @(
        @{
            Id   = 'R1'
            Rule = 'The ordinal is head + 1 at admission, assigned nowhere else.'
            Test = 'admission::tests::ordinals_are_admission_order_not_clock_or_id_order'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = '        let candidate = receipt::candidate(&mind, provenance, &strong_reads, &writes, receipt::head(self)? + 1, now)?;'
            New  = '        let candidate = receipt::candidate(&mind, provenance, &strong_reads, &writes, 1, now)?;'
        }
        @{
            Id   = 'R1L'
            Rule = 'The ordinal is admission order, not a function of the clock or the receipt id: a same-second batch of three must not fall back to ranking by (committed_at, receipt_id).'
            Test = 'admission::tests::ordinals_are_admission_order_not_clock_or_id_order'
            File = 'crates/huginn-mind/src/admission.rs'
            Old  = '        let candidate = receipt::candidate(&mind, provenance, &strong_reads, &writes, receipt::head(self)? + 1, now)?;'
            New  = @'
        let provisional = receipt::candidate(&mind, provenance.clone(), &strong_reads, &writes, 0, now)?;
        let mut order = self.receipts()?.into_iter().map(|r| (r.committed_at, r.receipt_id)).collect::<Vec<_>>();
        order.push((provisional.committed_at.clone(), provisional.receipt_id.clone()));
        order.sort();
        let ordinal = (order
            .iter()
            .position(|entry| entry == &(provisional.committed_at.clone(), provisional.receipt_id.clone()))
            .unwrap()
            + 1) as u64;
        let candidate = receipt::candidate(&mind, provenance, &strong_reads, &writes, ordinal, now)?;
'@
        }
        @{
            Id   = 'R1L2'
            Rule = 'head is exactly N, not N off by one.'
            Test = 'admission::tests::ordinals_are_admission_order_not_clock_or_id_order'
            File = 'crates/huginn-mind/src/receipt.rs'
            Old  = '        Ok(n)'
            New  = '        Ok(n.saturating_sub(1))'
        }
        @{
            Id   = 'R2'
            Rule = 'The ordinal is not digested (A9): an exact replay keeps the ordinal its first admission assigned, whatever the head has moved to since.'
            Test = 'admission::tests::an_exact_replay_keeps_its_ordinal'
            File = 'crates/huginn-mind/src/receipt.rs'
            Old  = '        let bytes = rmp_serde::to_vec_named(&(&self.instance, &self.strong_reads, &self.writes))'
            New  = '        let bytes = rmp_serde::to_vec_named(&(&self.instance, &self.strong_reads, &self.writes, self.ordinal))'
        }
        @{
            Id   = 'R3'
            Rule = 'head requires the ordinals to be exactly {1..=N}; a chain that is not dense refuses rather than answering a number.'
            Test = 'receipt::tests::a_chain_that_is_not_dense_refuses_admission_and_reads_alike'
            File = 'crates/huginn-mind/src/receipt.rs'
            Old  = @'
    let mut ordinals = mind.receipts()?.into_iter().map(|receipt| receipt.ordinal).collect::<Vec<_>>();
    ordinals.sort_unstable();
    let n = ordinals.len() as u64;
    if ordinals.into_iter().eq(1..=n) {
        Ok(n)
    } else {
        Err(MindRefusal::Unavailable { detail: "receipt ordinals are not 1..=N: found a duplicate or a gap".into() })
    }
'@
            New  = @'
    Ok(mind.receipts()?.len() as u64)
'@
        }
        @{
            Id    = 'R3L'
            Rule  = 'Density is validated wherever the ordinal is read, in the one shared head, not only along the one path a caller happens to exercise it from.'
            Test  = 'receipt::tests::a_chain_that_is_not_dense_refuses_admission_and_reads_alike'
            Edits = @(
                @{
                    File = 'crates/huginn-mind/src/receipt.rs'
                    Old  = @'
    let mut ordinals = mind.receipts()?.into_iter().map(|receipt| receipt.ordinal).collect::<Vec<_>>();
    ordinals.sort_unstable();
    let n = ordinals.len() as u64;
    if ordinals.into_iter().eq(1..=n) {
        Ok(n)
    } else {
        Err(MindRefusal::Unavailable { detail: "receipt ordinals are not 1..=N: found a duplicate or a gap".into() })
    }
'@
                    New  = @'
    Ok(mind.receipts()?.len() as u64)
'@
                }
                @{
                    File = 'crates/huginn-mind/src/admission.rs'
                    Old  = '        let candidate = receipt::candidate(&mind, provenance, &strong_reads, &writes, receipt::head(self)? + 1, now)?;'
                    New  = @'
        {
            let mut ordinals = self.receipts()?.into_iter().map(|r| r.ordinal).collect::<Vec<_>>();
            ordinals.sort_unstable();
            let n = ordinals.len() as u64;
            if !ordinals.into_iter().eq(1..=n) {
                return Err(MindRefusal::Unavailable {
                    detail: "receipt ordinals are not 1..=N: found a duplicate or a gap".into(),
                });
            }
        }
        let candidate = receipt::candidate(&mind, provenance, &strong_reads, &writes, receipt::head(self)? + 1, now)?;
'@
                }
            )
        }
        @{
            Id    = 'R4'
            Rule  = 'Receipt v2 is a new type id, not the old id with an added field: a store carrying a v1-shaped receipt is refused at the type gate.'
            Test  = 'mind::tests::a_v1_receipt_store_is_refused_at_open'
            Edits = @(
                @{
                    File = 'crates/huginn-mind/src/receipt.rs'
                    Old  = 'pub const RECEIPT_SCHEMA_VERSION: &str = "huginn.mind_commit_receipt.v2";'
                    New  = 'pub const RECEIPT_SCHEMA_VERSION: &str = "huginn.mind_commit_receipt.v1";'
                }
                @{
                    File = 'crates/huginn-mind/src/receipt.rs'
                    Old  = '#[cultcache(type = "huginn.mind_commit_receipt.v2", schema = "HuginnCommitReceipt")]'
                    New  = '#[cultcache(type = "huginn.mind_commit_receipt.v1", schema = "HuginnCommitReceipt")]'
                }
            )
        }
        @{
            Id   = 'R4L'
            Rule = 'The type gate knows only the current id, not the old one kept beside it for compatibility: a widened opener that still recognises huginn.mind_commit_receipt.v1 as known is the shape F3 warns against (a silent misread of every old receipt''s ordinal as 0), one step short of a full revert.'
            Test = 'mind::tests::a_v1_receipt_store_is_refused_at_open'
            File = 'crates/huginn-mind/src/mind.rs'
            Old  = @'
fn is_known_type(type_id: &str) -> bool {
    type_id == HuginnMindEpoch::TYPE
        || type_id == HuginnCommitReceipt::TYPE
        || PipelineKind::ALL.iter().any(|kind| kind.type_id() == type_id)
}
'@
            New  = @'
fn is_known_type(type_id: &str) -> bool {
    type_id == HuginnMindEpoch::TYPE
        || type_id == HuginnCommitReceipt::TYPE
        || type_id == "huginn.mind_commit_receipt.v1"
        || PipelineKind::ALL.iter().any(|kind| kind.type_id() == type_id)
}
'@
        }
        @{
            Id   = 'R5'
            Rule = 'The epoch gate runs before the type gate, so a store written at a foreign epoch is refused as ForeignEpoch and not ForeignStore: a real epoch bump always moves every type id, so the type gate would otherwise fire first and the epoch gate would never be reached for the one case it exists for.'
            Test = 'mind::tests::a_store_written_at_the_previous_epoch_is_refused_by_the_epoch_gate'
            File = 'crates/huginn-mind/src/mind.rs'
            Old  = @'
        refuse_foreign_epoch(&raw)?;
        refuse_foreign_types(&raw)?;
'@
            New  = @'
        refuse_foreign_types(&raw)?;
        refuse_foreign_epoch(&raw)?;
'@
        }
    )
}
