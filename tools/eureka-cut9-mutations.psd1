# Cut 9 mutations, V1-V24L: the read side's rules, each entry restoring one
# permissiveness the cut closed and naming the test that must fail while it is
# applied. A rule with two weakenings has two entries: `Vn` is the revert, the
# rule simply absent, and `VnL` is the loosening, the rule weakened rather than
# removed. Run through the Eureka skill's harness from this repo:
#
#   $env:CARGO_TARGET_DIR = 'C:\Users\Meta\.cargo-target-codex'
#   powershell -File C:\Users\Meta\.claude\skills\eureka\tools\eureka-mutations.ps1 -Repo F:\Projects\Huginn `
#       -Entries tools/eureka-cut9-mutations.psd1 `
#       -Target crates/huginn-mind/src/docs.rs,crates/huginn-mind/src/query.rs `
#       -Test 'cargo test -p huginn-mind --lib'
#
# `docs.rs` is a target for V1 alone, which is the point of V1: in force has
# one owner, and the mutant that breaks it for the rules is the mutant that
# breaks it for the views. V1L is H41's own edit, listed here as well as in
# `eureka-cut8-mutations.psd1` -- one mutant, two suites, one owner.
#
# Two weakenings the spec names have no entry, not because they are
# unreachable but because nothing in this suite exercises the path that would
# reach them. `MindStore::compare_and_swap_batch` is `pub`, and `store.rs`'s
# own re-export of `CacheBackingStore` already lets code outside this crate
# push a row that skips A5 (and every other admission rule) entirely -- the
# door named at `store.rs:18-19`. Recorded as not yet reached, not as
# unreachable:
#
# - `assignments_of` comparing the repo and ignoring the instance. A5 refuses
#   a stewardship naming another instance for every row admission writes, so
#   every stewardship admission ever lands already names the mind that holds
#   it, and every caller reachable through admission is a sequence this mind
#   derives over its own scope. A row planted through the store port directly,
#   naming another instance, is not yet reached by anything in this suite.
# - `resolutions_of` comparing a subject by `id` alone, which was `V10L` until
#   the leaf opened `PipelineRef::validate_ref` and `view` and `history` began
#   asking it. A ref that reaches the comparison through admission has a kind
#   its id's kind segment agrees with, and every stored subject admission
#   writes was held to the same grammar, so on both sides the kind is a
#   function of the id and comparing the whole ref and comparing the id alone
#   cannot differ for anything admission wrote. The rule is still the rule; it
#   is the reachable weakening that went, and what defends it is `V24`/`V24L`
#   at the doors instead. A row planted through the store port directly, with
#   a subject whose kind disagrees with its id, is not yet reached either.
#
# The one listed beside them in the spec is failable and has an entry: the
# `semantic` check below `Reader::new` changes the answer for a store the
# reader refuses -- a doubled receipt, a row that does not decode -- from the
# semantic refusal to the integrity one, and `V8L` is that move.
@{
    Mutations = @(
        @{
            Id   = 'V1'
            Rule = 'One owner for in force: the views derive status through the same closing_resolution the rules ask.'
            Test = 'query::tests::status_is_the_derivation_admission_uses'
            File = 'crates/huginn-mind/src/docs.rs'
            Old  = @'
        self.resolutions().find(|&(key, resolution)| {
            resolution.subject.kind == kind
                && resolution.subject.id.0 == id
                && !own(resolution)
                && self.in_force(PipelineKind::Resolution, key)
        })
'@
            New  = @'
        let _ = (kind, id, own);
        None
'@
        }
        @{
            Id   = 'V1L'
            Rule = 'In force is recursive for a reader as it is for a rule: a withdrawn resolution stops closing its subject. H41''s edit, run against the views.'
            Test = 'query::tests::status_is_the_derivation_admission_uses'
            File = 'crates/huginn-mind/src/docs.rs'
            Old  = @'
                && !own(resolution)
                && self.in_force(PipelineKind::Resolution, key)
'@
            New  = @'
                && !own(resolution)
                && { let _ = key; true }
'@
        }
        @{
            Id   = 'V2'
            Rule = 'admitted_at is the receipt''s committed_at, never the store''s stored_at stamp (R-C).'
            Test = 'query::tests::views_read_admitted_at_from_the_receipt_not_stored_at'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = '                    admitted_at: receipt.committed_at.clone(),'
            New  = '                    admitted_at: mind.raw_envelope(&write.document_type, &write.document_key).map_or_else(|| receipt.committed_at.clone(), |envelope| envelope.stored_at.clone()),'
        }
        @{
            Id   = 'V3'
            Rule = 'The facts come from a receipt''s writes only, so a document a later batch cited keeps the receipt that wrote it.'
            Test = 'query::tests::views_join_admission_facts_from_the_receipt_that_wrote_them'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = '            for write in receipt.writes.iter() {'
            New  = '            for write in receipt.writes.iter().chain(&receipt.strong_reads) {'
        }
        @{
            Id    = 'V4'
            Rule  = 'A row no receipt wrote refuses, from view and query alike, and is never skipped.'
            Test  = 'query::tests::views_join_admission_facts_from_the_receipt_that_wrote_them'
            Edits = @(
                @{
                    File = 'crates/huginn-mind/src/query.rs'
                    Old  = '            let view = reader.view_of(held)?;'
                    New  = '            let Ok(view) = reader.view_of(held) else { continue };'
                }
                @{
                    File = 'crates/huginn-mind/src/query.rs'
                    Old  = '        reader.held(id.kind, &id.id.0).map(|held| reader.view_of(held)).transpose()'
                    New  = '        Ok(reader.held(id.kind, &id.id.0).and_then(|held| reader.view_of(held).ok()))'
                }
            )
        }
        @{
            Id   = 'V4L'
            Rule = 'Two receipts naming one write is an integrity fault, not a choice of which to believe.'
            Test = 'query::tests::views_join_admission_facts_from_the_receipt_that_wrote_them'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = @'
                if facts.insert(identity, landed).is_some() {
                    return Err(integrity(&write.document_type, &write.document_key, "is written by two receipts"));
                }
'@
            New  = @'
                facts.insert(identity, landed);
'@
        }
        @{
            Id   = 'V5'
            Rule = 'The cap: a caller''s limit is clamped into 1..=QUERY_LIMIT_MAX.'
            Test = 'query::tests::query_orders_by_admitted_at_then_id_and_caps_at_200_with_the_match_count'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = '.clamp(1, QUERY_LIMIT_MAX));'
            New  = '.max(1));'
        }
        @{
            Id   = 'V5L'
            Rule = 'The cap is 200, and the test spells it out rather than reading the constant under test.'
            Test = 'query::tests::query_orders_by_admitted_at_then_id_and_caps_at_200_with_the_match_count'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = 'pub const QUERY_LIMIT_MAX: usize = 200;'
            New  = 'pub const QUERY_LIMIT_MAX: usize = 201;'
        }
        @{
            Id   = 'V6'
            Rule = 'The order is (admitted_at, id): the batch decides, and the key only breaks a tie inside one.'
            Test = 'query::tests::query_orders_by_admitted_at_then_id_and_caps_at_200_with_the_match_count'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = @'
        (&left.admission.admitted_at, &left.id.id.0).cmp(&(&right.admission.admitted_at, &right.id.id.0))
'@
            New  = @'
        left.id.id.0.cmp(&right.id.id.0)
'@
        }
        @{
            Id   = 'V6L'
            Rule = 'A tie inside one batch falls to the key, which is root-first and so is not the image''s (type, key) order.'
            Test = 'query::tests::query_orders_by_admitted_at_then_id_and_caps_at_200_with_the_match_count'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = @'
        (&left.admission.admitted_at, &left.id.id.0).cmp(&(&right.admission.admitted_at, &right.id.id.0))
'@
            New  = @'
        left.admission.admitted_at.cmp(&right.admission.admitted_at)
'@
        }
        @{
            Id   = 'V7'
            Rule = 'matched is the count before the cap, so a truncated page is visible as one.'
            Test = 'query::tests::query_orders_by_admitted_at_then_id_and_caps_at_200_with_the_match_count'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = @'
        let matched = items.len() as u32;
        ordered(&mut items);
        let limit = query.limit.map_or(QUERY_LIMIT_MAX, |limit| (limit as usize).clamp(1, QUERY_LIMIT_MAX));
        items.truncate(limit);
'@
            New  = @'
        ordered(&mut items);
        let limit = query.limit.map_or(QUERY_LIMIT_MAX, |limit| (limit as usize).clamp(1, QUERY_LIMIT_MAX));
        items.truncate(limit);
        let matched = items.len() as u32;
'@
        }
        @{
            Id   = 'V8'
            Rule = 'semantic is refused typed until Cut 11, never answered with an empty page.'
            Test = 'query::tests::semantic_query_refuses_typed_until_wired'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = @'
            return Err(MindRefusal::Unavailable {
                detail: "semantic query: the index is not wired (Cut 11)".into(),
            });
'@
            New  = @'
            return Ok(PipelineQueryPage { items: vec![], matched: 0 });
'@
        }
        @{
            Id   = 'V8L'
            Rule = 'The semantic refusal is decided before the image and the receipts are read, so a store the reader refuses still answers the question the caller asked.'
            Test = 'query::tests::semantic_query_refuses_typed_until_wired'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = @'
        if query.semantic.is_some() {
            return Err(MindRefusal::Unavailable {
                detail: "semantic query: the index is not wired (Cut 11)".into(),
            });
        }
        let reader = Reader::new(self)?;
'@
            New  = @'
        let reader = Reader::new(self)?;
        if query.semantic.is_some() {
            return Err(MindRefusal::Unavailable {
                detail: "semantic query: the index is not wired (Cut 11)".into(),
            });
        }
'@
        }
        @{
            Id   = 'V16'
            Rule = 'The cut filter is the label, not every cut.'
            Test = 'query::tests::query_filters_each_select_by_one_field'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = '            && !root_and_local(&base.key).1.starts_with(&format!("cut-{}.", cut.0))'
            New  = '            && !root_and_local(&base.key).1.starts_with("cut-")'
        }
        @{
            Id   = 'V16L'
            Rule = 'The cut prefix ends at the dot, so cut-1 does not match cut-10.'
            Test = 'query::tests::query_filters_each_select_by_one_field'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = '            && !root_and_local(&base.key).1.starts_with(&format!("cut-{}.", cut.0))'
            New  = '            && !root_and_local(&base.key).1.starts_with(&format!("cut-{}", cut.0))'
        }
        @{
            Id   = 'V17'
            Rule = 'A filter reaches a resolution through its base, the document it is a record of.'
            Test = 'query::tests::query_filters_each_select_by_one_field'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = @'
        let mut current = held;
        while let PipelineDocument::Resolution(resolution) = &current.document {
            let Some(subject) = self.held(resolution.subject.kind, &resolution.subject.id.0) else {
                return current;
            };
            current = subject;
        }
        current
'@
            New  = @'
        held
'@
        }
        @{
            Id   = 'V17L'
            Rule = 'The base is walked to the first non-resolution, not one step: a withdrawal of a resolution is two steps from its document.'
            Test = 'query::tests::query_filters_each_select_by_one_field'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = '        while let PipelineDocument::Resolution(resolution) = &current.document {'
            New  = '        if let PipelineDocument::Resolution(resolution) = &current.document {'
        }
        @{
            Id   = 'V18'
            Rule = 'campaign is the key''s root segment.'
            Test = 'query::tests::query_filters_each_select_by_one_field'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = @'
        if let Some(campaign) = &query.campaign
            && root_and_local(&held.key).0 != campaign.0
        {
            return false;
        }
'@
            New  = ''
        }
        @{
            Id   = 'V18L'
            Rule = 'The root, not a campaign field: a stewardship under an instance root and a resolution under its subject''s root carry no campaign of their own.'
            Test = 'query::tests::query_filters_each_select_by_one_field'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = '            && root_and_local(&held.key).0 != campaign.0'
            New  = '            && !matches!(&held.document, PipelineDocument::Campaign(value) if value.slug == *campaign)'
        }
        @{
            Id   = 'V19'
            Rule = 'kinds selects by the document''s kind, and an empty list matches every kind.'
            Test = 'query::tests::query_filters_each_select_by_one_field'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = @'
        if !query.kinds.is_empty() && !query.kinds.contains(&held.kind) {
            return false;
        }
'@
            New  = ''
        }
        @{
            Id   = 'V20'
            Rule = 'in_force selects by the derived status.'
            Test = 'query::tests::query_filters_each_select_by_one_field'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = @'
        if let Some(in_force) = query.in_force
            && in_force != (view.status == PipelineStatus::InForce)
        {
            return false;
        }
'@
            New  = ''
        }
        @{
            Id   = 'V20L'
            Rule = 'in_force selects the status the caller asked for, not its opposite.'
            Test = 'query::tests::query_filters_each_select_by_one_field'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = '            && in_force != (view.status == PipelineStatus::InForce)'
            New  = '            && in_force == (view.status == PipelineStatus::InForce)'
        }
        @{
            Id   = 'V21'
            Rule = 'faculty selects by the receipt''s provenance: attribution filters, and grants nothing (ruling 18).'
            Test = 'query::tests::query_filters_each_select_by_one_field'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = @'
        if let Some(faculty) = query.faculty
            && view.admission.provenance.faculty != faculty
        {
            return false;
        }
'@
            New  = ''
        }
        @{
            Id   = 'V23'
            Rule = 'repo selects by the repo the document names, read through the base.'
            Test = 'query::tests::query_filters_each_select_by_one_field'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = @'
        if let Some(repo) = &query.repo
            && !repo_matches(&base.document, repo)
        {
            return false;
        }
'@
            New  = ''
        }
        @{
            Id   = 'V23L'
            Rule = 'A campaign matches through the repos it lists, as the five single-repo kinds match through their field.'
            Test = 'query::tests::query_filters_each_select_by_one_field'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = '        D::Campaign(campaign) => campaign.repos.contains(repo),'
            New  = '        D::Campaign(_) => false,'
        }
        @{
            Id   = 'V24'
            Rule = 'The door that takes a ref asks the leaf''s grammar first: a ref whose kind and id disagree is refused, never answered over as an absent document. RS-2 deleted `history`, the rule''s other door; only `view`''s survives, so this is now a single edit rather than the two-door `Edits` array it was before.'
            Test = 'query::tests::a_ref_whose_kind_and_id_disagree_is_refused_by_both_doors'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = @'
        id.validate_ref()?;
        let reader = Reader::new(self)?;
'@
            New  = @'
        let reader = Reader::new(self)?;
'@
        }
    )
}
