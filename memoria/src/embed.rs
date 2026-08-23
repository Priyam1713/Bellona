//! Campaign X — Living Memory: embeddings abstraction, a local-first
//! cosine store, and hybrid keyword+vector recall (RRF fusion).

use crate::MemoriaError;
use async_trait::async_trait;

/// Text → dense vector. Providers live in `auxilia`; tests use the
/// deterministic `HashEmbedder`. Dimensions are pinned per collection
/// (mixing dimensions corrupts cosine math — refuse at insert).
#[async_trait]
pub trait Embedder: Send + Sync {
    fn dims(&self) -> usize;
    async fn embed(&self, text: &str) -> Result<Vec<f32>, MemoriaError>;
}

/// Deterministic bag-of-hashed-tokens embedder (default D=64), L2-normalized.
///
/// Not semantic — but stable, dependency-free, and perfect for proving the
/// pipeline. Swap in model-backed embedders without touching callers.
pub struct HashEmbedder {
    dims: usize,
}

impl HashEmbedder {
    pub fn new(dims: usize) -> Self {
        HashEmbedder { dims: dims.max(8) }
    }

    pub fn standard() -> Self {
        Self::new(64)
    }
}

#[async_trait]
impl Embedder for HashEmbedder {
    fn dims(&self) -> usize {
        self.dims
    }
    async fn embed(&self, text: &str) -> Result<Vec<f32>, MemoriaError> {
        let mut v = vec![0f32; self.dims];
        let lower = text.to_lowercase();
        for token in lower.split(|c: char| !(c.is_alphanumeric() || c == '-')) {
            if token.is_empty() {
                continue;
            }
            // FNV-1a for bucket + sign
            let mut h: u64 = 0xcbf29ce484222325;
            for b in token.as_bytes() {
                h ^= *b as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
            let bucket = (h % self.dims as u64) as usize;
            let sign = if (h >> 32) & 1 == 0 { 1.0 } else { -1.0 };
            v[bucket] += sign;
        }
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut v {
                *x /= norm;
            }
        }
        Ok(v)
    }
}

/// Cosine similarity over equal-length vectors (guarding zero norms).
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return -1.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return -1.0;
    }
    dot / (na * nb)
}

// ---------- the vector store ----------

/// One stored vector with its source episode reference.
#[derive(Debug, Clone)]
pub struct StoredVector {
    pub episode_id: String,
    pub kind: String,
    pub content: String,
    pub dim: usize,
    pub blob: Vec<u8>, // little-endian f32s
}

pub fn pack(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for f in v {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

pub fn unpack(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}
