use serde::Deserialize;
use serde_json::Value;

use crate::providers::gemini::response::GeminiInteractionsResponseTokenUsage;

#[derive(Debug, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum GeminiChunkEvent {
    #[serde(rename = "interaction.created")]
    InteractionCreated {
        interaction: GeminiInteractionInfo,
        metadata: Option<GeminiEventMetadata>,
    },
    #[serde(rename = "step.start")]
    StartDelta {
        index: Option<usize>,
        step: Option<GeminiStart>,
    },

    #[serde(rename = "step.delta")]
    StepDelta {
        index: Option<usize>,
        delta: Option<GeminiDelta>,
    },

    #[serde(rename = "step.stop")]
    StopDelta {
        index: Option<usize>,
        metadata: Option<GeminiEventMetadata>,
    },
    #[serde(rename = "interaction.completed")]
    InteractionCompleted {
        interaction: Option<GeminiInteractionInfo>,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
pub struct GeminiInteractionInfo {
    pub id: String,
    pub status: String,
    pub model: String,
    pub usage: Option<GeminiInteractionsResponseTokenUsage>,
}

#[derive(Debug, Deserialize)]
pub struct GeminiStart {
    pub id: Option<String>,
    pub r#type: String,
    pub name: Option<String>,
    pub arguments: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct GeminiDelta {
    pub index: Option<usize>,
    pub r#type: String,
    pub arguments: Option<String>,
    pub content: Option<GeminiDeltaContent>, // for thought_summary
    pub text: Option<String>,                // for text
    pub signature: Option<String>,           // for signature
}

#[derive(Debug, Deserialize)]
pub struct GeminiDeltaContent {
    pub text: Option<String>,
    pub r#type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GeminiEventMetadata {
    pub total_usage: GeminiInteractionsResponseTokenUsage,
}
