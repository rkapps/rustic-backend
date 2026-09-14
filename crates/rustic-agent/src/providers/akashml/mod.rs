pub mod completion;

pub const LLM: &str = "AkashML";
pub const MODEL_GLM_5P3: &str = "glm-5.3";
const AKASHML_BASE_URL: &str = "https://api.akashml.com/v1";

/// Return the list of supported GPT model identifiers.
pub fn models() -> Vec<String> {
    vec![MODEL_GLM_5P3.to_string()]
}
