//! `huginn import <campaign> --from <instance> --from-endpoint <rudp>`: the receiver's Self replays
//! a moved campaign's receipts from the giver's mind into her own.
//!
//! The verb decides only what to send and in what order. `huginn_mind::plan` lays the replay out
//! from one cursor walk of the giver; the receiver's walk of her own root says what she already
//! holds, so a rerun after any stop resumes at the first unheld batch. Every write is the
//! receiver daemon's `Admit`, one per source receipt, and her admission decides it. The giver is
//! sent `Query` alone. Nothing is stored here: progress is what the receiver holds, and one
//! verification walk of her root at the end compares every planned document.

use std::collections::{BTreeMap, BTreeSet};

use cultnet_rs::{LIMIT_MAX, PROJECTION_DOCUMENT, Selection};
use eureka_state::hold;
use huginn_mind::eureka_pipeline::{PipelineDocument, PipelineRef, Short, Slug};
use huginn_mind::{
    Faculty, HuginnMindRequest, HuginnMindResponse, ImportBatch, ImportFault, ImportPlan, PipelineAdmissionBatch, PipelineAdmissionOutcome,
    PipelineDocumentSummary, PipelineDocumentView, PipelinePageItems, PipelineProvenance, PipelineSelectionPage, plan,
};

use crate::config::{Settings, TOOL};
use crate::trouble::Trouble;
use crate::{differences, say, to_value, unexpected};

/// The arguments of the verb, as given.
pub struct Flags {
    pub campaign: String,
    pub from: String,
    pub from_endpoint: String,
    pub faculty: Faculty,
    pub agent: String,
    pub session: String,
}

/// Why the verb stops: the mind said no (exit 1, the text printed) or a trouble (its own exit).
enum Stop {
    No(String),
    Trouble(Trouble),
}

impl From<Trouble> for Stop {
    fn from(trouble: Trouble) -> Self {
        Self::Trouble(trouble)
    }
}

type Step<T> = Result<T, Stop>;

pub fn run(settings: &Settings, flags: Flags) -> Result<u8, Trouble> {
    match import(settings, flags) {
        Ok(()) => Ok(0),
        Err(Stop::No(text)) => {
            say(text);
            Ok(1)
        }
        Err(Stop::Trouble(trouble)) => Err(trouble),
    }
}

fn import(settings: &Settings, flags: Flags) -> Step<()> {
    let campaign = Slug(flags.campaign.clone());
    campaign.validate_slug().map_err(|refusal| Trouble::invalid(format!("the campaign is not a valid name: {refusal:?}")))?;
    let giver = settings.source(&flags.from, &flags.from_endpoint)?;
    let receiver = settings.client()?.instance().clone();

    let views = documents(&|selection| {
        giver.call(HuginnMindRequest::Query { instance: giver.instance().clone(), selection, semantic: None }).map_err(Trouble::from)
    }, &campaign)?;
    let plan = plan(&campaign, giver.instance(), &receiver, views).map_err(fault)?;

    let ask_receiver = |selection: Selection| {
        settings.ask(|instance| HuginnMindRequest::Query { instance: instance.clone(), selection, semantic: None })
    };
    let held = resume(&plan, &headers(&ask_receiver, &campaign)?, giver.instance())?;
    let (receipts, total) = (plan.batches.len(), plan.batches.iter().map(|batch| batch.documents.len()).sum::<usize>());
    say(format_args!(
        "plan {campaign} from {giver}: {receipts} receipts {total} documents, {held} held, moved by {moved}",
        campaign = flags.campaign,
        giver = flags.from,
        held = held.iter().filter(|held| **held).count(),
        moved = plan.moved.id.0,
    ));

    for (index, (batch, held)) in plan.batches.iter().zip(&held).enumerate() {
        if *held {
            continue;
        }
        let external = unheld(&ask_receiver, &batch.foreign)?;
        let count = external.len();
        send(settings, &flags, batch, external)?;
        say(format_args!("imported ordinal {} {} documents {count} external ({}/{receipts})", batch.source.ordinal, batch.documents.len(), index + 1));
    }

    verify(&documents(&|selection| ask_receiver(selection), &campaign)?, &plan, giver.instance())?;
    say(format_args!("imported {} from {}: {receipts} receipts {total} documents, verified", flags.campaign, flags.from));
    Ok(())
}

/// A planner fault as the verb ends: the mind's no for who the campaign moved to, an integrity
/// trouble for a history that cannot be replayed whole.
fn fault(fault: ImportFault) -> Stop {
    match fault {
        ImportFault::NotMoved | ImportFault::MovedElsewhere { .. } | ImportFault::SourceIsReceiver => Stop::No(format!("Refused {fault:?}")),
        ImportFault::BatchTooLarge { .. } | ImportFault::ExternalsTooMany { .. } | ImportFault::Unordered { .. } => {
            Stop::Trouble(Trouble::Integrity(format!("{fault:?}; nothing was written")))
        }
    }
}

/// One batch, admitted as the importer's own with the source's receipt in `imported_from`.
fn send(settings: &Settings, flags: &Flags, batch: &ImportBatch, external: Vec<PipelineRef>) -> Step<()> {
    let mut source = batch.source.clone();
    source.external = external;
    let provenance = PipelineProvenance {
        faculty: flags.faculty,
        agent: Short(flags.agent.clone()),
        session: Short(flags.session.clone()),
        tool: Short(TOOL.to_string()),
        imported_from: Some(source),
    };
    let documents = batch.documents.iter().map(|(_, document)| document.clone()).collect();
    let ordinal = batch.source.ordinal;
    match settings.ask(|instance| HuginnMindRequest::Admit(PipelineAdmissionBatch { instance: instance.clone(), provenance, documents }))? {
        HuginnMindResponse::Admit(PipelineAdmissionOutcome::Committed { .. } | PipelineAdmissionOutcome::AlreadyAdmitted { .. }) => Ok(()),
        HuginnMindResponse::Admit(PipelineAdmissionOutcome::Refused(refusal)) | HuginnMindResponse::Refused(refusal) => {
            Err(Stop::No(format!("Refused {refusal:?} at source ordinal {ordinal}")))
        }
        HuginnMindResponse::Admit(PipelineAdmissionOutcome::Conflict { identities }) => Err(Stop::No(format!(
            "Conflict at source ordinal {ordinal}: {}",
            identities.iter().map(|identity| identity.id.0.as_str()).collect::<Vec<_>>().join(", ")
        ))),
        other => Err(unexpected(&other).into()),
    }
}

/// The foreign referents the receiver does not hold: exactly the ones the batch declares external,
/// since declaring one she holds is refused.
fn unheld(ask: &dyn Fn(Selection) -> Result<HuginnMindResponse, Trouble>, foreign: &[PipelineRef]) -> Step<Vec<PipelineRef>> {
    let mut held = BTreeSet::new();
    for chunk in foreign.chunks(LIMIT_MAX as usize) {
        let selection = Selection {
            keys: Some(chunk.iter().map(|referent| referent.id.0.clone()).collect()),
            limit: Some(chunk.len() as u32),
            ..Selection::default()
        };
        for page in pages(ask, selection)? {
            let PipelinePageItems::Headers(found) = page.items else { return Err(Trouble::Internal("a header query answered documents".into()).into()) };
            held.extend(found.into_iter().map(|header| header.id.id.0));
        }
    }
    Ok(foreign.iter().filter(|referent| !held.contains(&referent.id.0)).cloned().collect())
}

/// Every page of a selection, to the end, each as of the first page's snapshot.
fn pages(ask: &dyn Fn(Selection) -> Result<HuginnMindResponse, Trouble>, mut selection: Selection) -> Step<Vec<PipelineSelectionPage>> {
    let mut found = Vec::new();
    loop {
        match ask(selection.clone())? {
            HuginnMindResponse::Query(page) => {
                let next = page.next.clone();
                found.push(page);
                match next {
                    Some(next) => selection.cursor = Some(next),
                    None => return Ok(found),
                }
            }
            HuginnMindResponse::Refused(refusal) => return Err(Stop::No(format!("Refused {refusal:?}"))),
            other => return Err(unexpected(&other).into()),
        }
    }
}

fn root_walk(campaign: &Slug, projection: &str) -> Selection {
    Selection {
        projection: projection.to_string(),
        limit: Some(LIMIT_MAX),
        ..hold::any_of(Selection::default(), "root", &[&campaign.0])
    }
}

/// Every document under the campaign's root, whole.
fn documents(ask: &dyn Fn(Selection) -> Result<HuginnMindResponse, Trouble>, campaign: &Slug) -> Step<Vec<PipelineDocumentView>> {
    let mut views = Vec::new();
    for page in pages(ask, root_walk(campaign, PROJECTION_DOCUMENT))? {
        let PipelinePageItems::Documents(found) = page.items else { return Err(Trouble::Internal("a document query answered headers".into()).into()) };
        views.extend(found);
    }
    Ok(views)
}

/// Every document under the campaign's root, as a header.
fn headers(ask: &dyn Fn(Selection) -> Result<HuginnMindResponse, Trouble>, campaign: &Slug) -> Step<Vec<PipelineDocumentSummary>> {
    let mut headers = Vec::new();
    for page in pages(ask, root_walk(campaign, &Selection::default().projection))? {
        let PipelinePageItems::Headers(found) = page.items else { return Err(Trouble::Internal("a header query answered documents".into()).into()) };
        headers.extend(found);
    }
    Ok(headers)
}

/// Which batches the receiver already holds whole. A document under the root that is not one of
/// the plan's, imported from the plan's receipt of the source, is her own record and refuses the
/// import; a batch held in part cannot be resumed, only repaired.
fn resume(plan: &ImportPlan, root: &[PipelineDocumentSummary], source: &Slug) -> Step<Vec<bool>> {
    let planned: BTreeMap<&str, (usize, &str)> = plan
        .batches
        .iter()
        .enumerate()
        .flat_map(|(index, batch)| batch.documents.iter().map(move |(id, _)| (id.id.0.as_str(), (index, batch.source.receipt_id.0.as_str()))))
        .collect();
    let mut held: Vec<BTreeSet<&str>> = vec![BTreeSet::new(); plan.batches.len()];
    let mut own = Vec::new();
    for header in root {
        let key = header.id.id.0.as_str();
        let imported = header.admission.provenance.imported_from.as_ref();
        match planned.get(key) {
            Some((index, receipt)) if imported.is_some_and(|from| from.instance == *source && from.receipt_id.0 == *receipt) => {
                held[*index].insert(key);
            }
            _ => own.push(key),
        }
    }
    if !own.is_empty() {
        return Err(Stop::No(format!("Refused the receiver already holds records under the root that are not imported from this source: {}; nothing was written", own.join(", "))));
    }
    plan.batches
        .iter()
        .zip(&held)
        .map(|(batch, held)| match held.len() {
            0 => Ok(false),
            count if count == batch.documents.len() => Ok(true),
            count => Err(Stop::Trouble(Trouble::Integrity(format!(
                "source ordinal {} is held in part ({count} of {} documents); repair it by hand, a rerun cannot",
                batch.source.ordinal,
                batch.documents.len()
            )))),
        })
        .collect()
}

/// The receiver's root against the plan: every planned document equal and imported from its
/// source receipt, and nothing else under the root.
fn verify(root: &[PipelineDocumentView], plan: &ImportPlan, source: &Slug) -> Step<()> {
    let held: BTreeMap<&str, &PipelineDocumentView> = root.iter().map(|view| (view.id.id.0.as_str(), view)).collect();
    let mut differing = Vec::new();
    for batch in &plan.batches {
        for (id, document) in &batch.documents {
            let key = id.id.0.as_str();
            let Some(view) = held.get(key) else {
                differing.push(format!("{key}: missing from the receiver"));
                continue;
            };
            let mut what = Vec::new();
            differences(&to_value(document), &to_value(&view.document), "", &mut what);
            let imported = view.admission.provenance.imported_from.as_ref();
            if imported.is_none_or(|from| from.instance != *source || from.receipt_id != batch.source.receipt_id) {
                what.push("imported_from".to_string());
            }
            if !what.is_empty() {
                differing.push(format!("{key}: {}", if what.iter().all(|path| path.is_empty()) { "the document".to_string() } else { what.join(", ") }));
            }
        }
    }
    let planned: BTreeSet<&str> = plan.batches.iter().flat_map(|batch| batch.documents.iter().map(|(id, _)| id.id.0.as_str())).collect();
    differing.extend(held.keys().filter(|key| !planned.contains(*key)).map(|key| format!("{key}: not in the plan")));
    if differing.is_empty() {
        Ok(())
    } else {
        Err(Stop::Trouble(Trouble::ReadBackDiffers(format!("the receiver's root differs from the giver's: {}", differing.join("; ")))))
    }
}

#[cfg(test)]
mod tests {
    use huginn_mind::eureka_pipeline::{Date, PipelineKind, PipelineRuling, RulingAuthority};
    use huginn_mind::{AdmissionFacts, ImportSource, PipelineFacts, PipelineStatus, PipelineStatusSummary};

    use super::*;

    fn slug(text: &str) -> Slug {
        Slug(text.to_string())
    }

    fn id(local: &str) -> PipelineRef {
        PipelineRef { kind: PipelineKind::Ruling, id: Short(format!("c:ruling:{local}")) }
    }

    fn batch(ordinal: u64, locals: &[&str]) -> ImportBatch {
        let source = ImportSource {
            instance: slug("eureka"),
            receipt_id: Short(format!("receipt-{ordinal}")),
            ordinal,
            committed_at: Short("2026-11-08T16:00:00Z".into()),
            faculty: Faculty::Hands,
            agent: Short("a".into()),
            session: Short("s".into()),
            tool: Short("huginn".into()),
            external: vec![],
        };
        ImportBatch { source, documents: locals.iter().map(|local| (id(local), question(local))).collect(), foreign: vec![] }
    }

    fn question(local: &str) -> PipelineDocument {
        PipelineDocument::Ruling(PipelineRuling {
            campaign: slug("c"),
            label: local.into(),
            title: "A ruling".into(),
            answers: None,
            choice: None,
            ruling: "It stands.".into(),
            operator_quote: None,
            ruled_on: Date("2026-11-08".into()),
            precedents: vec![],
            authority: RulingAuthority::Standing,
        })
    }

    fn plan_of(batches: Vec<ImportBatch>) -> ImportPlan {
        ImportPlan { moved: id("moved"), batches }
    }

    fn header(local: &str, from: Option<(&str, u64)>) -> PipelineDocumentSummary {
        let imported_from = from.map(|(instance, ordinal)| ImportSource { instance: slug(instance), ..batch(ordinal, &[]).source });
        PipelineDocumentSummary {
            id: id(local),
            admission: AdmissionFacts {
                receipt_id: "mine".into(),
                admitted_at: "2026-11-08T17:00:00Z".into(),
                provenance: PipelineProvenance { faculty: Faculty::Hands, agent: Short("me".into()), session: Short("s".into()), tool: Short("huginn".into()), imported_from },
                ordinal: 1,
            },
            status: PipelineStatusSummary::InForce,
            facts: PipelineFacts::Campaign { title: "t".into(), repos: vec![] },
        }
    }

    fn two() -> ImportPlan {
        plan_of(vec![batch(3, &["a", "b"]), batch(5, &["c"])])
    }

    fn stopped(step: Step<Vec<bool>>) -> String {
        match step {
            Err(Stop::No(text)) => format!("no: {text}"),
            Err(Stop::Trouble(trouble)) => format!("{trouble}"),
            Ok(held) => panic!("{held:?}"),
        }
    }

    #[test]
    fn a_rerun_resumes_at_the_first_unheld_batch() {
        let source = slug("eureka");
        assert_eq!(resume(&two(), &[], &source).ok().unwrap(), vec![false, false]);
        let first = [header("a", Some(("eureka", 3))), header("b", Some(("eureka", 3)))];
        assert_eq!(resume(&two(), &first, &source).ok().unwrap(), vec![true, false], "the held receipt is skipped");
        let both = [first.as_slice(), &[header("c", Some(("eureka", 5)))]].concat();
        assert_eq!(resume(&two(), &both, &source).ok().unwrap(), vec![true, true]);
    }

    #[test]
    fn a_receipt_held_in_part_is_an_integrity_fault() {
        let one_of_two = [header("a", Some(("eureka", 3)))];
        let said = stopped(resume(&two(), &one_of_two, &slug("eureka")));
        assert!(said.starts_with("Integrity: source ordinal 3 is held in part (1 of 2"), "{said}");
    }

    #[test]
    fn the_receivers_own_records_under_the_root_refuse_the_import() {
        let source = slug("eureka");
        for own in [
            header("zz", None),
            header("a", None),
            header("a", Some(("kiko", 3))),
            header("a", Some(("eureka", 5))),
        ] {
            let said = stopped(resume(&two(), &[own], &source));
            assert!(said.starts_with("no: Refused the receiver already holds records under the root"), "{said}");
        }
    }

    fn viewed(local: &str, document: PipelineDocument, from: Option<(&str, u64)>) -> PipelineDocumentView {
        let held = header(local, from);
        PipelineDocumentView { id: held.id, document, admission: held.admission, status: PipelineStatus::InForce }
    }

    #[test]
    fn the_verification_walk_names_every_difference() {
        let source = slug("eureka");
        let plan = plan_of(vec![batch(3, &["a", "b"]), batch(5, &["c"])]);
        let ours = |local: &str, ordinal| viewed(local, question(local), Some(("eureka", ordinal)));
        let right = vec![ours("a", 3), ours("b", 3), ours("c", 5)];
        assert!(verify(&right, &plan, &source).is_ok());

        let mut changed = question("b");
        if let PipelineDocument::Ruling(ruling) = &mut changed {
            ruling.title = "Reworded".into();
        }
        let wrong = vec![ours("a", 3), viewed("b", changed, Some(("eureka", 3))), viewed("c", question("c"), Some(("eureka", 3))), ours("x", 5)];
        let Err(Stop::Trouble(trouble)) = verify(&wrong, &plan, &source) else { panic!("a difference is exit 5") };
        assert_eq!(trouble.exit(), 5);
        let said = trouble.to_string();
        for named in ["c:ruling:b: value.title", "c:ruling:c: imported_from", "c:ruling:x: not in the plan"] {
            assert!(said.contains(named), "{named} in {said}");
        }
        assert!(!said.contains("c:ruling:a:"), "{said}");

        let missing = vec![ours("a", 3), ours("b", 3)];
        let Err(Stop::Trouble(trouble)) = verify(&missing, &plan, &source) else { panic!() };
        assert!(trouble.to_string().contains("c:ruling:c: missing from the receiver"), "{trouble}");
    }
}
