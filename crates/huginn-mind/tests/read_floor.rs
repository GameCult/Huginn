//! What a read costs, measured through the public API alone so the same file
//! runs at the base and at the branch. Ignored: it wants a copy of a real
//! store and says what it measured instead of asserting.
//!
//! `FLOOR_STATE` is a directory holding `minds/<instance>/mind.redb`. It is
//! copied to a temp dir and never opened in place. `FLOOR_SCALE` (default 1)
//! grows the copy with synthetic history to that multiple of its documents;
//! `FLOOR_ROOTS` is `spread` (each hundred units under its own root) or
//! `same` (one root grows). `FLOOR_INSTANCE` defaults to `eureka`; `FLOOR_SAMPLES` (default 7) is how many times each read is timed, for a base too slow to sample 7 times.
//!
//! cargo test --release -p huginn-mind --features test-support --test read_floor -- --ignored --nocapture
#![cfg(feature = "test-support")]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use chrono::Utc;
use cultnet_rs::{Citation, FieldPredicate, Incoming, RecordRef, Selection};
use huginn_mind::eureka_pipeline::{PipelineDocument, PipelineKind, PipelinePulse, PipelineRef, Short, Slug};
use huginn_mind::oracle::synth_history_at;
use huginn_mind::{Faculty, IndexStatus, Mind, PipelineAdmissionBatch, PipelinePageItems, PipelineProvenance};

fn env(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

fn rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status.lines().find_map(|line| line.strip_prefix("VmRSS:")?.split_whitespace().next()?.parse().ok())
        })
        .unwrap_or(0)
}

fn any_of(index: &str, values: &[&str]) -> FieldPredicate {
    FieldPredicate { index: index.into(), op: "any_of".into(), values: Some(values.iter().map(|v| v.to_string()).collect()), number: None }
}

fn of(kind: PipelineKind, fields: Vec<FieldPredicate>) -> Selection {
    Selection { schemas: Some(vec![kind.type_id().to_string()]), fields: Some(fields), ..Selection::default() }
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

fn time<T>(mut read: impl FnMut() -> T) -> Duration {
    let samples: usize = env("FLOOR_SAMPLES", "7").parse().unwrap();
    median(
        (0..samples)
            .map(|_| {
                let started = Instant::now();
                std::hint::black_box(read());
                started.elapsed()
            })
            .collect(),
    )
}

fn keys_of(mind: &Mind<huginn_mind::OwnedRedbMessagePackBackingStore>, kind: PipelineKind) -> Vec<PipelineRef> {
    let page = mind.query(&Selection { schemas: Some(vec![kind.type_id().to_string()]), limit: Some(5), ..Selection::default() }).unwrap();
    match page.items {
        PipelinePageItems::Headers(headers) => headers.into_iter().map(|header| header.id).collect(),
        PipelinePageItems::Documents(views) => views.into_iter().map(|view| view.id).collect(),
    }
}

#[test]
#[ignore]
fn read_floor() {
    let instance = Slug(env("FLOOR_INSTANCE", "eureka"));
    let scale: u32 = env("FLOOR_SCALE", "1").parse().unwrap();
    let spread = env("FLOOR_ROOTS", "spread") == "spread";
    let source = PathBuf::from(env("FLOOR_STATE", "/state")).join("minds").join(&instance.0).join("mind.redb");
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("minds").join(&instance.0);
    std::fs::create_dir_all(&target).unwrap();
    std::fs::copy(&source, target.join("mind.redb")).unwrap();
    let path = target.join("mind.redb");

    let synthetic = if scale > 1 { synth_history_at(&path, &instance, scale, spread).unwrap() } else { 0 };
    let size_mib = std::fs::metadata(&path).unwrap().len() / (1024 * 1024);

    let started = Instant::now();
    let mut mind = Mind::open(temp.path(), &instance).unwrap();
    let open = started.elapsed();
    let rss = rss_kib() / 1024;
    let status = mind.status(IndexStatus::Current);

    let findings = keys_of(&mind, PipelineKind::Finding);
    let finding = findings.first().expect("the store holds a finding").clone();
    let root = finding.id.0.split(':').next().unwrap().to_string();
    let mut one = Selection { keys: Some(vec![finding.id.0.clone()]), projection: "document".into(), ..Selection::default() };
    one.schemas = Some(vec![PipelineKind::Finding.type_id().to_string()]);

    let run_desc = Selection { descending: true, limit: Some(3), ..of(PipelineKind::Run, vec![any_of("root", &[&root]), any_of("operated_by", &["Mind"])]) };
    let cites = Selection {
        cites: Some(Citation { target: RecordRef::new(PipelineKind::Finding.type_id(), finding.id.0.clone()), role: Some("subject".into()) }),
        ..of(PipelineKind::Resolution, vec![])
    };
    let stewardship = of(PipelineKind::Stewardship, vec![any_of("root", &[&root])]);
    let memory = of(PipelineKind::Memory, vec![any_of("root", &[&root]), any_of("memory_origin", &["Room"])]);
    let follow_up = Selection { limit: Some(20), ..of(PipelineKind::FollowUp, vec![any_of("in_force", &["true"])]) };
    let three = of(
        PipelineKind::Finding,
        vec![any_of("severity", &["High"]), any_of("confidence", &["Confirmed"]), any_of("origin", &["Introduced"])],
    );
    let unreported = Selection {
        cited: Some(Incoming { role: "cut_spec".into(), exists: false }),
        ..of(PipelineKind::CutSpec, vec![any_of("in_force", &["true"])])
    };

    let paged = |selection: &Selection| {
        let mut selection = selection.clone();
        let mut pages = 0;
        loop {
            let page = mind.query(&selection).unwrap();
            pages += 1;
            match page.next {
                Some(next) if pages < 3 => selection.cursor = Some(next),
                _ => return pages,
            }
        }
    };

    let mut rows: Vec<(&str, Duration)> = vec![
        ("whoami/status", time(|| mind.status(IndexStatus::Current))),
        ("persona", time(|| mind.persona().unwrap())),
        ("view", time(|| mind.view(&finding).unwrap())),
        ("keys=1 document", time(|| mind.query(&one).unwrap())),
        ("run root+operated_by desc", time(|| mind.query(&run_desc).unwrap())),
        ("resolution cites subject", time(|| mind.query(&cites).unwrap())),
        ("stewardship root", time(|| mind.query(&stewardship).unwrap())),
        ("memory two fields", time(|| mind.query(&memory).unwrap())),
        ("follow_up in_force paged", time(|| paged(&follow_up))),
        ("finding three fields", time(|| mind.query(&three).unwrap())),
        ("cut_spec in_force cited", time(|| mind.query(&unreported).unwrap())),
    ];
    let line = [&run_desc, &cites, &stewardship, &memory, &follow_up, &three, &unreported, &one];
    let first_line = time(|| {
        for n in 0..25 {
            let _ = mind.query(line[n % line.len()]);
        }
    });
    rows.push(("first line (25 reads)", first_line));

    let pulse = PipelineDocument::Pulse(PipelinePulse {
        instance: instance.clone(),
        label: format!("floor-{}", Utc::now().timestamp_nanos_opt().unwrap_or_default()).as_str().into(),
        note: "read floor".into(),
        tempo_vote_beat_s: None,
        tempo_vote_withdrawn: false,
        urgency_permille: Some(1),
    });
    let provenance = PipelineProvenance { faculty: Faculty::Hands, agent: Short("floor".into()), session: Short("floor".into()), tool: Short("floor".into()) };
    let started = Instant::now();
    let outcome = mind.admit(PipelineAdmissionBatch { instance: instance.clone(), provenance, documents: vec![pulse] }, Utc::now());
    let admit = started.elapsed();
    let started = Instant::now();
    let read_back = mind.read_back();
    let read_back_time = started.elapsed();

    println!("\nFLOOR documents={} receipts={} store_mib={} synthetic_docs={} scale={} roots={}", status.documents, status.receipts, size_mib, synthetic, scale, if spread { "spread" } else { "same" });
    println!("FLOOR open_ms={:.0} rss_mib={}", open.as_secs_f64() * 1e3, rss);
    for (name, took) in rows {
        println!("FLOOR read {:<28} {:>10.1} us", name, took.as_secs_f64() * 1e6);
    }
    println!("FLOOR admit_ms={:.0} ({})", admit.as_secs_f64() * 1e3, if matches!(outcome, huginn_mind::PipelineAdmissionOutcome::Committed { .. }) { "committed".to_string() } else { format!("{outcome:?}") });
    println!("FLOOR read_back_ms={:.0} ({:?})", read_back_time.as_secs_f64() * 1e3, read_back);
}
