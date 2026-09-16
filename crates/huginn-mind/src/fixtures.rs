//! Test fixtures shared by the modules' tests: one valid document of each
//! kind the tests need, prepared through the leaf against the organ's cache.

use cultcache_rs::CultCacheEnvelope;
use epiphany_pipeline::{Date, PipelineDocument, PipelineInstance, Short, Slug};

use crate::mind::{HuginnMindEpoch, schema_cache};

pub(crate) const INSTANCE: &str = "yggdrasil";

pub(crate) fn slug(value: &str) -> Slug {
    value.into()
}

pub(crate) fn s(value: &str) -> Short {
    value.into()
}

pub(crate) fn date() -> Date {
    Date("2026-09-16".into())
}

pub(crate) fn instance(name: &str) -> PipelineDocument {
    PipelineDocument::Instance(PipelineInstance {
        instance: slug(name),
        display_name: s(&format!("{name} mind")),
        created_at: date(),
        host: s(name),
    })
}

/// The leaf's envelope for a document, keyed and encoded as the organ stores
/// it.
pub(crate) fn prepare(document: &PipelineDocument) -> CultCacheEnvelope {
    document.validate().unwrap();
    document.prepare(&schema_cache().unwrap()).unwrap()
}

/// The epoch record admission derives on the first write.
pub(crate) fn epoch() -> CultCacheEnvelope {
    HuginnMindEpoch::envelope(&schema_cache().unwrap()).unwrap()
}
