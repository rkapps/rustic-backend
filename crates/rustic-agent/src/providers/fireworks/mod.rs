pub mod completion;

pub const LLM: &str = "Fireworks";
pub const MODEL_GLM_5P2: &str = "accounts/fireworks/models/glm-5p2";
const FIREWORKS_BASE_URL: &str = "https://api.fireworks.ai/inference/v1";

/// Return the list of supported GPT model identifiers.
pub fn models() -> Vec<String> {
    vec![MODEL_GLM_5P2.to_string()]
}
