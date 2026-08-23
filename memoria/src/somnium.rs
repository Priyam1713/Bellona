//! Somnium — sleep-time consolidation.
//!
//! Like the research frontier's "sleep-time compute": during idle windows a
//! daemon distills raw episodic history into durable facts and reusable
//! skills, keeping the smallest memory that improves outcomes.

use crate::archivum::{new_episode, ArchivumStore, Episode};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

/// What consolidation produces.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DistilledKnowledge {
    pub facts: Vec<String>,
    pub skills: Vec<String>,
}

/// Consolidation strategy. Swap heuristic → LLM-backed without touching
/// callers.
#[async_trait]
pub trait Consolidator: Send + Sync {
    async fn consolidate(&self, episodes: &[Episode]) -> DistilledKnowledge;
}

/// Deterministic, dependency-free distillation:
/// - repeated line prefixes become skills (procedural),
/// - unique statements become facts (semantic).
///
/// Honest about its simplicity; it exists so the pipeline is real from day
/// one and swappable forever.
pub struct HeuristicConsolidator;

#[async_trait]
impl Consolidator for HeuristicConsolidator {
    async fn consolidate(&self, episodes: &[Episode]) -> DistilledKnowledge {
        let mut freq: BTreeMap<String, usize> = BTreeMap::new();
        let mut facts: Vec<String> = Vec::new();
        for e in episodes {
            // Procedural signal: "how we did X" style records repeat.
            if e.kind == "procedural" || e.kind == "episodic" {
                *freq.entry(e.content.clone()).or_insert(0) += 1;
            } else {
                facts.push(e.content.clone());
            }
        }
        let skills: Vec<String> = freq
            .into_iter()
            .filter(|(_, n)| *n >= 2)
            .map(|(c, _)| c)
            .collect();
        DistilledKnowledge { facts, skills }
    }
}

/// The idle-time loop. Every `interval`, consolidate new episodes into
/// distilled knowledge written back as `kind = "distilled"`.
pub struct SleepDaemon;

impl SleepDaemon {
    /// Run one consolidation pass; returns how many distilled entries written.
    pub async fn pass(
        store: &dyn ArchivumStore,
        consolidator: &dyn Consolidator,
    ) -> Result<usize, crate::MemoriaError> {
        let mut episodes = Vec::new();
        for kind in ["episodic", "procedural", "semantic"] {
            episodes.extend(store.by_kind(kind).await?);
        }
        let knowledge = consolidator.consolidate(&episodes).await;
        let existing_distilled = store.by_kind("distilled").await?;
        let mut n = 0;
        for f in &knowledge.facts {
            let already = existing_distilled.iter().any(|e| &e.content == f);
            if !already {
                store.put(new_episode("distilled", f.clone())).await?;
                n += 1;
            }
        }
        for s in &knowledge.skills {
            let content = format!("skill: {s}");
            if !existing_distilled.iter().any(|e| e.content == content) {
                store.put(new_episode("distilled", content)).await?;
                n += 1;
            }
        }
        Ok(n)
    }

    /// Spawn the daemon on tokio. Cancel the task to stop; it is stateless.
    pub fn spawn(
        store: Arc<dyn ArchivumStore>,
        consolidator: Arc<dyn Consolidator>,
        interval: Duration,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                let _ = Self::pass(store.as_ref(), consolidator.as_ref()).await;
            }
        })
    }
}
