//! Contract suite — TIME-AWARE RECALL.
//!
//! Companion to `recency_decay_red.rs`. Where that suite proves decay is
//! recency-aware, this one proves the RECALL PATH itself is recency-aware at
//! query time: ranking must factor in elapsed time since `last_accessed`
//! WITHOUT relying on a prior `apply_decay` pass. A fresh memory must out-rank a
//! staler one even when the stale one carries a higher stored weight and no
//! decay has run.
//!
//! These tests exercise only the public `MemoryStore` API and the public
//! `Memory` fields. Each sets up rows identical except for recency, drives a
//! recall path, and asserts the fresh-first contract.

use chrono::{Duration, Utc};
use icm_core::memory::{Importance, Memory};
use icm_core::store::MemoryStore;
use icm_store::SqliteStore;

/// Build a Medium memory aged `age_days` in the past (created + last_accessed),
/// with a fixed starting weight, zero access count, and the given keywords.
/// Everything except recency is held constant so any recall-order difference is
/// attributable solely to elapsed time.
fn aged_memory(
    topic: &str,
    summary: &str,
    keywords: &[&str],
    age_days: i64,
    weight: f32,
) -> Memory {
    let mut m = Memory::new(topic.to_string(), summary.to_string(), Importance::Medium);
    let t = Utc::now() - Duration::days(age_days);
    m.created_at = t;
    m.updated_at = t;
    m.last_accessed = t;
    m.weight = weight;
    m.access_count = 0;
    m.keywords = keywords.iter().map(|k| k.to_string()).collect();
    m
}

/// CONTRACT 1 (core): keyword recall ranks a fresh memory above a stale one
/// WITHOUT any decay pass — even though the stale memory has the HIGHER stored
/// weight, so a weight-only ranking would put it first.
#[test]
fn keyword_recall_ranks_fresh_above_stale_without_decay() {
    let store = SqliteStore::in_memory().expect("open in-memory store");

    // Stale starts HIGHER so today's weight-only ORDER BY would rank it first.
    store
        .store(aged_memory(
            "recall-stale",
            "shared recall keyword stale",
            &["convergence"],
            400,
            1.0,
        ))
        .expect("store stale");
    store
        .store(aged_memory(
            "recall-fresh",
            "shared recall keyword fresh",
            &["convergence"],
            0,
            0.8,
        ))
        .expect("store fresh");

    // NOTE: no apply_decay() is called — recency must be applied at query time.
    let hits = store
        .search_by_keywords(&["convergence"], 10)
        .expect("keyword search");

    assert!(hits.len() >= 2, "both seeded memories must match the query");
    assert_eq!(
        hits[0].topic, "recall-fresh",
        "recall contract: a fresh memory must out-rank a 400-day-stale, \
         higher-weight one at query time with no decay pass (got top topic = {})",
        hits[0].topic
    );
}

/// CONTRACT 2: among equal-weight memories, keyword recall order is monotonic
/// in staleness — fresher ranks earlier.
#[test]
fn keyword_recall_orders_by_recency_when_weight_equal() {
    let store = SqliteStore::in_memory().expect("open in-memory store");

    store
        .store(aged_memory(
            "age-180",
            "recency ranked entry",
            &["ranked"],
            180,
            1.0,
        ))
        .expect("store 180d");
    store
        .store(aged_memory(
            "age-0",
            "recency ranked entry",
            &["ranked"],
            0,
            1.0,
        ))
        .expect("store 0d");
    store
        .store(aged_memory(
            "age-30",
            "recency ranked entry",
            &["ranked"],
            30,
            1.0,
        ))
        .expect("store 30d");

    let hits = store
        .search_by_keywords(&["ranked"], 10)
        .expect("keyword search");

    let order: Vec<&str> = hits.iter().map(|m| m.topic.as_str()).collect();
    assert_eq!(
        order,
        vec!["age-0", "age-30", "age-180"],
        "recall contract: equal-weight memories must recall fresh-first; got {order:?}"
    );
}

/// CONTRACT 3: topic recall (`get_by_topic`) applies the same recency ranking —
/// the fresh entry surfaces first even from a lower stored weight, no decay.
#[test]
fn topic_recall_ranks_fresh_above_stale_without_decay() {
    let store = SqliteStore::in_memory().expect("open in-memory store");

    store
        .store(aged_memory("proj", "stale topic entry here", &[], 365, 1.0))
        .expect("store stale");
    store
        .store(aged_memory("proj", "fresh topic entry here", &[], 0, 0.85))
        .expect("store fresh");

    let hits = store.get_by_topic("proj").expect("topic recall");

    assert!(hits.len() >= 2, "both topic entries must be returned");
    assert_eq!(
        hits[0].summary, "fresh topic entry here",
        "recall contract: topic recall must surface the fresh entry first \
         (got top summary = {})",
        hits[0].summary
    );
}
