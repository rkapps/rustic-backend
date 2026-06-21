
pub mod completion;

pub const LLM: &str = "Groq";

pub const MODEL_GWEN3_32B: &str = "qwen/qwen3-32b";
pub const MODEL_GWEN36_27B: &str = "qwen/qwen3.6-27b";
pub const MODEL_LLAMA_33_70B: &str = "llama-3.3-70b-versatile";
const GROQ_BASE_URL: &str = "https://api.groq.com/openai";

/// Return the list of supported GPT model identifiers.
pub fn models() -> Vec<String> {
    vec![
        MODEL_GWEN3_32B.to_string(),
        MODEL_GWEN3_32B.to_string(),
        MODEL_GWEN3_32B.to_string(),
    ]
}
