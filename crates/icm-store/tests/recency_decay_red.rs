//! RED suite — convergence contract: DYNAMIC, TIME-AWARE (recency) importance/decay.
//!
//! Cycle-7 planning target = icm. The convergence gap (trend-researcher):
//! importance/decay is STATIC. `SqliteStore::apply_decay` multiplies every
//! non-critical row's weight by a flat factor derived ONLY from `importance`
//! and `access_count` — it never reads `last_accessed` / `created_at`. So a
//! memory last touched a year ago and one touched a second ago decay
//! identically. For the lifeos meta front-door union (icm <-> handoff <->
//! rusty-idd) a memory surfaced into a handoff capsule MUST be recency-weighted
//! (Ebbinghaus / recency): staleness is a function of elapsed wall-clock time.
//!
//! These tests are ADDITIVE and exercise ONLY the existing public API
//! (`MemoryStore` + `SqliteStore::in_memory` + the public `Memory` fields).
//! They are RED today because the time-aware-decay capability is genuinely
//! ABSENT — not because of a compile error or a wrong API call. Each test sets
//! up rows that are IDENTICAL except for `last_accessed`, drives the existing
//! decay path, and asserts the recency contract the GREEN implementation owes.
//!
//! Traceability: contract "recency-aware decay/recall" -> each `#[test]` below.

use chrono::{Duration, Utc};
use icm_core::memory::{Importance, Memory};
use icm_core::store::MemoryStore;
use icm_store::SqliteStore;

/// Build a Medium-importance memory whose timestamps are `age_days` in the
/// past, with a known starting weight and zero access count. Everything except
/// recency is held constant so any post-decay divergence is attributable
/// solely to elapsed time since `last_accessed`.
fn aged_memory(topic: &str, summary: &str, age_days: i64, weight: f32) -> Memory {
    let mut m = Memory::new(topic.to_string(), summary.to_string(), Importance::Medium);
    let t = Utc::now() - Duration::days(age_days);
    m.created_at = t;
    m.updated_at = t;
    m.last_accessed = t;
    m.weight = weight;
    m.access_count = 0;
    m
}

/// CONTRACT 1 (core): with importance and access_count held equal, a stale
/// memory must lose MORE weight under decay than a freshly-accessed one.
/// Today both decay by the same flat factor -> equal weights -> RED.
#[test]
fn decay_stale_memory_loses_more_weight_than_fresh() {
    let store = SqliteStore::in_memory().expect("open in-memory store");

    let fresh_id = store
        .store(aged_memory("recency-fresh", "fresh convergence memory", 0, 1.0))
        .expect("store fresh");
    let stale_id = store
        .store(aged_memory("recency-stale", "stale convergence memory", 365, 1.0))
        .expect("store stale");

    // One pass of the standard daily decay factor.
    store.apply_decay(0.95).expect("apply decay");

    let fresh = store.get(&fresh_id).expect("get fresh").expect("fresh row");
    let stale = store.get(&stale_id).expect("get stale").expect("stale row");

    assert!(
        stale.weight < fresh.weight,
        "recency contract: a 365-day-stale memory must decay MORE than a fresh one \
         (stale={}, fresh={}); decay is currently time-blind",
        stale.weight,
        fresh.weight
    );
}

/// CONTRACT 2: decay magnitude must be MONOTONIC in staleness. Three otherwise
/// identical memories aged 0 / 30 / 180 days must end with strictly decreasing
/// weight. Today all three share one flat factor -> all equal -> RED.
#[test]
fn decay_magnitude_is_monotonic_in_staleness() {
    let store = SqliteStore::in_memory().expect("open in-memory store");

    let id0 = store
        .store(aged_memory("mono-0d", "convergence age zero", 0, 1.0))
        .expect("store 0d");
    let id30 = store
        .store(aged_memory("mono-30d", "convergence age thirty", 30, 1.0))
        .expect("store 30d");
    let id180 = store
        .store(aged_memory("mono-180d", "convergence age one eighty", 180, 1.0))
        .expect("store 180d");

    store.apply_decay(0.95).expect("apply decay");

    let w0 = store.get(&id0).unwrap().unwrap().weight;
    let w30 = store.get(&id30).unwrap().unwrap().weight;
    let w180 = store.get(&id180).unwrap().unwrap().weight;

    assert!(
        w0 > w30 && w30 > w180,
        "recency contract: weight after decay must strictly decrease with staleness \
         (0d={w0}, 30d={w30}, 180d={w180}); decay is currently time-blind"
    );
}

/// CONTRACT 3 (recall re-rank): a fresh memory must out-rank a STALER one even
/// when the stale one starts with a slightly higher static weight. We seed the
/// stale memory at 1.0 and the fresh one at 0.9, then decay and recall via the
/// existing weight-ordered keyword search. A recency-aware decay drops the
/// 2-year-stale memory below the fresh one, so the fresh memory ranks first.
/// Today the flat factor preserves the initial ordering -> stale ranks first
/// -> RED. Deterministic (no tie: initial weights differ).
#[test]
fn recall_ranks_fresh_above_stale_after_decay() {
    let store = SqliteStore::in_memory().expect("open in-memory store");

    // Stale starts HIGHER so today's time-blind decay keeps it on top.
    store
        .store(aged_memory(
            "rank-stale",
            "shared convergence keyword stale",
            730,
            1.0,
        ))
        .expect("store stale");
    store
        .store(aged_memory(
            "rank-fresh",
            "shared convergence keyword fresh",
            0,
            0.9,
        ))
        .expect("store fresh");

    store.apply_decay(0.95).expect("apply decay");

    let hits = store
        .search_by_keywords(&["convergence"], 10)
        .expect("keyword search");

    assert!(hits.len() >= 2, "both seeded memories must match the query");
    assert_eq!(
        hits[0].topic, "rank-fresh",
        "recency contract: a fresh memory must out-rank a 2-year-stale one after decay, \
         even from a lower starting weight; recall ranking is currently recency-blind \
         (got top topic = {})",
        hits[0].topic
    );
}

/// CONTRACT 4 (recency floor): a memory accessed RIGHT NOW should barely decay
/// in a single pass — elapsed time ~ 0 implies forgetting ~ 0. Today the flat
/// 0.95 factor drops a Medium memory to ~0.95 regardless of recency, so it
/// falls below the floor -> RED.
#[test]
fn fresh_memory_decay_is_negligible() {
    let store = SqliteStore::in_memory().expect("open in-memory store");

    let id = store
        .store(aged_memory("floor-fresh", "just accessed convergence memory", 0, 1.0))
        .expect("store fresh");

    store.apply_decay(0.95).expect("apply decay");

    let w = store.get(&id).unwrap().unwrap().weight;
    assert!(
        w > 0.99,
        "recency contract: a memory accessed now must barely decay in one pass \
         (got weight={w}); decay ignores how recently the memory was accessed"
    );
}

/// CONTRACT 5 (magnitude reflects elapsed time): a very stale memory (~400
/// days untouched) must lose substantial weight in a single decay pass that
/// represents accrued forgetting. Today one `apply_decay(0.95)` yields ~0.95
/// for a Medium memory no matter how old -> not below the 0.7 forgetting
/// threshold -> RED.
#[test]
fn very_stale_memory_decays_substantially() {
    let store = SqliteStore::in_memory().expect("open in-memory store");

    let id = store
        .store(aged_memory(
            "magnitude-stale",
            "long untouched convergence memory",
            400,
            1.0,
        ))
        .expect("store stale");

    store.apply_decay(0.95).expect("apply decay");

    let w = store.get(&id).unwrap().unwrap().weight;
    assert!(
        w < 0.7,
        "recency contract: a ~400-day-untouched memory must decay substantially \
         in one pass (got weight={w}); decay magnitude ignores elapsed time"
    );
}
