pub mod completion;

pub const LLM: &str = "Mistral";
pub const MODEL_MISTRAL_SMALL: &str = "mistral-small-latest";
const MISTRAL_BASE_URL: &str = "https://api.mistral.ai/v1";

/// Return the list of supported GPT model identifiers.
pub fn models() -> Vec<String> {
    vec![
        MODEL_MISTRAL_SMALL.to_string(),
    ]
}
