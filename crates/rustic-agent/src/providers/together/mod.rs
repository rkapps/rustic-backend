pub mod completion;

pub const LLM: &str = "Together";

pub const MODEL_GLM_5_2: &str = "zai-org/GLM-5.2";
pub const MODEL_KIMI_K2_7_CODE: &str = "moonshotai/Kimi-K2.7-Code";
pub const MODEL_MINIMAX_M3: &str = "MiniMaxAI/MiniMax-M3";
const TOGETHER_BASE_URL: &str = "https://api.together.ai/v1";

/// Return the list of supported GPT model identifiers.
pub fn models() -> Vec<String> {
    vec![
        MODEL_GLM_5_2.to_string(),
        MODEL_KIMI_K2_7_CODE.to_string(),
        MODEL_MINIMAX_M3.to_string(),
    ]
}
