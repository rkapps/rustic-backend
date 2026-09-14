pub mod completion;

pub const LLM: &str = "AsiOne";
pub const MODEL_ASI1: &str = "asi1";
pub const MODEL_ASI_MINI: &str = "asi1_mini";
pub const MODEL_ASI_ULTRA: &str = "asi1_ultra";
const ASIONE_BASE_URL: &str = "https://api.asi1.ai";

/// Return the list of supported GPT model identifiers.
pub fn models() -> Vec<String> {
    vec![MODEL_ASI1.to_string(), MODEL_ASI_MINI.to_string(), MODEL_ASI_ULTRA.to_string()]
}

