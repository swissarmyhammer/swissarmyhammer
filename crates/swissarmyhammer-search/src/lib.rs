pub mod tokenize;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Doc {
    pub id: String,
    pub fields: Vec<Field>,
    pub embedding: Option<Vec<f32>>,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub weight: f32,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct Query {
    pub text: String,
    pub embedding: Option<Vec<f32>>,
    pub weights: SignalWeights,
    pub top_k: usize,
    pub min_score: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct SignalWeights {
    pub w_bm25: f32,
    pub w_trigram: f32,
    pub w_cosine: f32,
}

impl Default for SignalWeights {
    fn default() -> Self {
        Self {
            w_bm25: 1.0,
            w_trigram: 1.0,
            w_cosine: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hit {
    pub id: String,
    pub score: f32,
    pub signals: Signals,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Signals {
    pub bm25: f32,
    pub trigram: f32,
    pub cosine: f32,
}
