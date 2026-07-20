//! Google Gemini provider: request mapping to the Interactions API, streaming SSE parsing, and response normalisation.

pub mod chunk;
pub mod completion;
pub mod helper;
pub mod request;
pub mod response;

/// Provider identifier returned by [`models`] callers for display or routing.
pub const LLM: &str = "Gemini";

pub const MODEL_GEMINI_3_FLASH_PREVIEW: &str = "gemini-3-flash-preview";
pub const MODEL_GEMINI_3_1_FLASH_LITE_PREVIEW: &str = "gemini-3.1-flash-lite-preview";
pub const MODEL_GEMINI_3_1_PRO_PREVIEW: &str = "gemini-3.1-pro-preview";
pub const MODEL_GEMINI_EMBEDDING_001: &str = "gemini-embedding-001";

const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com";

/// Return the list of supported Gemini model identifiers.
pub fn models() -> Vec<String> {
    vec![
        MODEL_GEMINI_3_FLASH_PREVIEW.to_string(),
        MODEL_GEMINI_3_1_FLASH_LITE_PREVIEW.to_string(),
        MODEL_GEMINI_3_1_PRO_PREVIEW.to_string(),
    ]
}
