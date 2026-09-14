pub mod completion;

pub const LLM: &str = "Cerebras";
pub const MODEL_QWEN_3P8_27B: &str = "qwen-3.8-27b";
const CEREBRAS_BASE_URL: &str = "https://api.cerebras.ai/v1";

/// Return the list of supported GPT model identifiers.
pub fn models() -> Vec<String> {
    vec![MODEL_QWEN_3P8_27B.to_string()]
}
