use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Number of days over which a memory's recall recency halves: the recency
/// multiplier is `1.0` at day 0, ~`0.5` at 30 days, ~`0.25` at 90 days. Shared
/// by [`Memory::recency_factor`] and the wake-up scorer so "how stale is this
/// memory" means one thing across ICM.
pub const RECENCY_HALF_LIFE_DAYS: f32 = 30.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub access_count: u32,
    pub weight: f32,

    pub topic: String,
    pub summary: String,
    pub raw_excerpt: Option<String>,
    pub keywords: Vec<String>,

    pub importance: Importance,
    pub source: MemorySource,

    pub related_ids: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,

    /// Cloud scope: user (local default), project, or org.
    #[serde(default)]
    pub scope: Scope,
}

impl Memory {
    /// Build the text used for embedding this memory.
    pub fn embed_text(&self) -> String {
        format!("{} {}", self.topic, self.summary)
    }

    pub fn new(topic: String, summary: String, importance: Importance) -> Self {
        let now = Utc::now();
        Self {
            id: ulid::Ulid::new().to_string(),
            created_at: now,
            updated_at: now,
            last_accessed: now,
            access_count: 0,
            weight: 1.0,
            topic,
            summary,
            raw_excerpt: None,
            keywords: Vec::new(),
            importance,
            source: MemorySource::Manual,
            related_ids: Vec::new(),
            embedding: None,
            scope: Scope::User,
        }
    }

    /// Reference timestamp for recency scoring: the more recent of creation and
    /// last access, so a frequently-recalled memory stays "fresh".
    pub fn recency_reference(&self) -> DateTime<Utc> {
        self.created_at.max(self.last_accessed)
    }

    /// Query-time recency multiplier in `(0, 1]`: `1.0` for a just-touched
    /// memory, ~`0.5` at [`RECENCY_HALF_LIFE_DAYS`], ~`0.25` at triple that.
    /// Shared by recall ranking and the wake-up pack. Clamped at 0 elapsed days
    /// so a future timestamp (clock skew) scores as fresh rather than boosting
    /// the multiplier above 1.0.
    pub fn recency_factor(&self, now: DateTime<Utc>) -> f32 {
        let days = (now - self.recency_reference()).num_days().max(0) as f32;
        1.0 / (1.0 + days / RECENCY_HALF_LIFE_DAYS)
    }

    /// Recall-ranking weight: stored [`weight`](Self::weight) scaled by
    /// [`recency_factor`](Self::recency_factor). A fresh memory out-ranks an
    /// equally- or higher-weighted staler one at query time, independent of
    /// when `apply_decay` last ran.
    pub fn effective_weight(&self, now: DateTime<Utc>) -> f32 {
        self.weight * self.recency_factor(now)
    }
}

/// Memory scope for cloud sync.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// Personal memories (local only, default).
    #[default]
    User,
    /// Shared within a project (synced to cloud).
    Project,
    /// Shared across the entire organization (synced to cloud).
    Org,
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::Project => write!(f, "project"),
            Self::Org => write!(f, "org"),
        }
    }
}

impl std::str::FromStr for Scope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user" => Ok(Self::User),
            "project" => Ok(Self::Project),
            "org" => Ok(Self::Org),
            _ => Err(format!("invalid scope: {s} (expected: user, project, org)")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Importance {
    Critical,
    High,
    Medium,
    Low,
}

impl fmt::Display for Importance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Critical => write!(f, "critical"),
            Self::High => write!(f, "high"),
            Self::Medium => write!(f, "medium"),
            Self::Low => write!(f, "low"),
        }
    }
}

impl std::str::FromStr for Importance {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "critical" => Ok(Self::Critical),
            "high" => Ok(Self::High),
            "medium" => Ok(Self::Medium),
            "low" => Ok(Self::Low),
            _ => Err(format!("invalid importance: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MemorySource {
    ClaudeCode {
        session_id: String,
        file_path: Option<String>,
    },
    Conversation {
        thread_id: String,
    },
    Manual,
}

impl fmt::Display for MemorySource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClaudeCode { session_id, .. } => write!(f, "claude-code:{session_id}"),
            Self::Conversation { thread_id } => write!(f, "conversation:{thread_id}"),
            Self::Manual => write!(f, "manual"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StoreStats {
    pub total_memories: usize,
    pub total_topics: usize,
    pub avg_weight: f32,
    pub oldest_memory: Option<DateTime<Utc>>,
    pub newest_memory: Option<DateTime<Utc>>,
}

/// A cluster of related memories detected by keyword similarity analysis.
#[derive(Debug, Clone)]
pub struct PatternCluster {
    /// A representative summary for the cluster (from the highest-weight memory).
    pub representative_summary: String,
    /// IDs of memories in this cluster.
    pub memory_ids: Vec<String>,
    /// Common keywords shared across the cluster.
    pub keywords: Vec<String>,
    /// Number of memories in the cluster.
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TopicHealth {
    pub topic: String,
    pub entry_count: usize,
    pub avg_weight: f32,
    pub avg_access_count: f32,
    pub oldest: Option<DateTime<Utc>>,
    pub newest: Option<DateTime<Utc>>,
    pub last_accessed: Option<DateTime<Utc>>,
    pub needs_consolidation: bool,
    pub stale_count: usize,
}

impl TopicHealth {
    pub fn status(&self) -> &'static str {
        if self.needs_consolidation && self.stale_count > 0 {
            "!! NEEDS ATTENTION"
        } else if self.needs_consolidation {
            "!  consolidate"
        } else if self.stale_count > 0 {
            "-  has stale entries"
        } else {
            "ok healthy"
        }
    }
}
