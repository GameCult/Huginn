# Cut 9 mutations, V1-V23: the read side's rules, each entry restoring one
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
# Two weakenings the spec names have no entry because they change no behaviour
# this Body can reach, and a mutation that cannot fail is not a proof:
#
# - `assignments_of` comparing the repo and ignoring the instance. A5 refuses
#   a stewardship naming another instance, so every stewardship a mind holds
#   already names that mind, and every other caller is a sequence this mind
#   derives over its own scope.
# - `resolutions_of` comparing a subject by `id` alone, which was `V10L` until
#   the leaf opened `PipelineRef::validate_ref` and `view` and `history` began
#   asking it. A ref that reaches the comparison now has a kind its id's kind
#   segment agrees with, and every stored subject was held to the same grammar
#   at admission, so on both sides the kind is a function of the id and
#   comparing the whole ref and comparing the id alone cannot differ. The rule
#   is still the rule; it is the reachable weakening that went, and what
#   defends it is `V24`/`V24L` at the doors instead.
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
            Id   = 'V9'
            Rule = 'History is in the scope''s own sequence order, not the image''s key order.'
            Test = 'query::tests::a_subjects_history_lists_every_resolution_with_its_status_and_receipt'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = '        records.sort_by_key(|(sequence, _)| *sequence);'
            New  = '        let _ = &records;'
        }
        @{
            Id   = 'V10'
            Rule = 'A subject''s history is selected by the subject field, so a resolution''s own withdrawals are not in it.'
            Test = 'query::tests::a_subjects_history_lists_every_resolution_with_its_status_and_receipt'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = @'
                    .resolutions_of(subject)
                    .map(|(key, resolution)| (resolution.sequence, key.to_string()))
'@
            New  = @'
                    .resolutions()
                    .map(|(key, resolution)| { let _ = subject; (resolution.sequence, key.to_string()) })
'@
        }
        @{
            Id   = 'V11'
            Rule = 'A repo''s history is selected by (this mind, this repo), so another repo''s assignments are another history.'
            Test = 'query::tests::a_repos_stewardship_history_on_a_mind_lists_every_assignment'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = @'
                    .assignments_of(reader.mind.instance(), repo)
                    .map(|(key, stewardship)| (stewardship.sequence, key.to_string()))
'@
            New  = @'
                    .stewardships()
                    .map(|(key, stewardship)| { let _ = repo; (stewardship.sequence, key.to_string()) })
'@
        }
        @{
            Id   = 'V12'
            Rule = 'Open questions, findings and follow-ups are the ones in force.'
            Test = 'query::tests::open_items_are_derived_from_in_force_and_citation'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = '            of_kind(kind).filter(|view| view.status == PipelineStatus::InForce).cloned().collect::<Vec<_>>()'
            New  = '            of_kind(kind).cloned().collect::<Vec<_>>()'
        }
        @{
            Id   = 'V13'
            Rule = 'A spec without a report is open only while it is in force: a superseded revision is not open work.'
            Test = 'query::tests::open_items_are_derived_from_in_force_and_citation'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = '            specs_without_report: in_force(PipelineKind::CutSpec)'
            New  = '            specs_without_report: of_kind(PipelineKind::CutSpec).cloned().collect::<Vec<_>>()'
        }
        @{
            Id   = 'V13L'
            Rule = 'A spec is reported by a report naming its exact id, not by any report of its cut: a report of the revision this one superseded is not this revision''s report.'
            Test = 'query::tests::open_items_are_derived_from_in_force_and_citation'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = '                .filter(|view| !reported.contains(&view.id.id.0.as_str()))'
            New  = '                .filter(|view| !reported.iter().any(|cited| root_and_local(cited).1.split(''.'').next() == root_and_local(&view.id.id.0).1.split(''.'').next()))'
        }
        @{
            Id   = 'V14'
            Rule = 'A report is closed by the verdict that names it; a report is not resolvable, so nothing else closes it.'
            Test = 'query::tests::open_items_are_derived_from_in_force_and_citation'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = '                .filter(|view| !judged.contains(&view.id.id.0.as_str()))'
            New  = '                .filter(|view| { let _ = &judged; true })'
        }
        @{
            Id   = 'V14L'
            Rule = 'The citation is compared as the exact id, not as the cut it belongs to: one cut may have two reports and one verdict.'
            Test = 'query::tests::open_items_are_derived_from_in_force_and_citation'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = '                .filter(|view| !judged.contains(&view.id.id.0.as_str()))'
            New  = '                .filter(|view| !judged.iter().any(|cited| root_and_local(cited).1.split(''.'').next() == root_and_local(&view.id.id.0).1.split(''.'').next()))'
        }
        @{
            Id   = 'V15'
            Rule = 'Open items are one campaign''s, selected by the key''s root.'
            Test = 'query::tests::open_items_are_derived_from_in_force_and_citation'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = '            .filter(|view| root_and_local(&view.id.id.0).0 == campaign.0)'
            New  = '            .filter(|view| { let _ = &campaign; true })'
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
            Id    = 'V22'
            Rule  = 'The admission window selects by admitted_at.'
            Test  = 'query::tests::query_filters_each_select_by_one_field'
            Edits = @(
                @{
                    File = 'crates/huginn-mind/src/query.rs'
                    Old  = @'
        if let Some(after) = &query.admitted_after
            && view.admission.admitted_at <= after.0
        {
            return false;
        }
'@
                    New  = ''
                }
                @{
                    File = 'crates/huginn-mind/src/query.rs'
                    Old  = @'
        if let Some(before) = &query.admitted_before
            && view.admission.admitted_at >= before.0
        {
            return false;
        }
'@
                    New  = ''
                }
            )
        }
        @{
            Id    = 'V22L'
            Rule  = 'Both ends of the window are exclusive at the exact boundary.'
            Test  = 'query::tests::query_filters_each_select_by_one_field'
            Edits = @(
                @{
                    File = 'crates/huginn-mind/src/query.rs'
                    Old  = '            && view.admission.admitted_at <= after.0'
                    New  = '            && view.admission.admitted_at < after.0'
                }
                @{
                    File = 'crates/huginn-mind/src/query.rs'
                    Old  = '            && view.admission.admitted_at >= before.0'
                    New  = '            && view.admission.admitted_at > before.0'
                }
            )
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
            Rule = 'Every door that takes a ref asks the leaf''s grammar first: a ref whose kind and id disagree is refused, never answered over as an absent document or an empty history.'
            Test = 'query::tests::a_ref_whose_kind_and_id_disagree_is_refused_by_both_doors'
            Edits = @(
                @{
                    File = 'crates/huginn-mind/src/query.rs'
                    Old  = @'
        id.validate_ref()?;
        let reader = Reader::new(self)?;
'@
                    New  = @'
        let reader = Reader::new(self)?;
'@
                }
                @{
                    File = 'crates/huginn-mind/src/query.rs'
                    Old  = @'
        if let HistoryScope::Subject(subject) = scope {
            subject.validate_ref()?;
        }
'@
                    New  = @'
        if let HistoryScope::Subject(subject) = scope {
            let _ = subject;
        }
'@
                }
            )
        }
        @{
            Id   = 'V24L'
            Rule = 'Both doors ask, not one: a mind that validates the ref it views and not the subject it takes a history of still answers an empty history over a ref that is no ref.'
            Test = 'query::tests::a_ref_whose_kind_and_id_disagree_is_refused_by_both_doors'
            File = 'crates/huginn-mind/src/query.rs'
            Old  = @'
        if let HistoryScope::Subject(subject) = scope {
            subject.validate_ref()?;
        }
'@
            New  = @'
        if let HistoryScope::Subject(subject) = scope {
            let _ = subject;
        }
'@
        }
    )
}
