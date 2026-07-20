use serde_json::Value;

use crate::{
    CompletionResponseTokenUsage, providers::gemini::response::GeminiInteractionsResponseTokenUsage,
};

pub fn clean_for_gemini(params: &serde_json::Value) -> serde_json::Value {
    match params {
        serde_json::Value::Object(map) => {
            let cleaned = map
                .iter()
                .filter(|(k, _)| {
                    !matches!(
                        k.as_str(),
                        "additionalProperties"
                            | "exclusiveMinimum"
                            | "exclusiveMaximum"
                            | "$schema"
                            | "$id"
                            | "minLength"
                            | "maxLength"
                            | "prefill"
                            | "enumTitles"
                            | "examples"
                            | "default"
                            | "minimum"
                            | "maximum"
                            | "pattern"
                    )
                })
                .map(|(k, v)| {
                    // clean newlines from string values (descriptions)
                    let cleaned_v = if k == "description" {
                        if let serde_json::Value::String(s) = v {
                            serde_json::Value::String(
                                s.replace('\n', " ")
                                    .split_whitespace()
                                    .collect::<Vec<&str>>()
                                    .join(" "),
                            )
                        } else {
                            clean_for_gemini(v)
                        }
                    } else {
                        clean_for_gemini(v)
                    };
                    (k.clone(), cleaned_v)
                })
                .collect();
            serde_json::Value::Object(cleaned)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(clean_for_gemini).collect())
        }
        other => other.clone(),
    }
}

pub fn to_completion_reponse_token_usage(
    cusage: GeminiInteractionsResponseTokenUsage,
) -> CompletionResponseTokenUsage {
    CompletionResponseTokenUsage {
        input_tokens: cusage.total_input_tokens - cusage.total_cached_tokens,
        cached_read_tokens: cusage.total_cached_tokens,
        cached_write_tokens: 0,
        tool_use_tokens: cusage.total_tool_use_tokens,
        output_tokens: cusage.total_output_tokens, // Gemini already excludes thought tokens here
        reasoning_tokens: cusage.total_thought_tokens,
        total_tokens: (cusage.total_input_tokens - cusage.total_cached_tokens)  // fresh input
                                 + cusage.total_cached_tokens                                         // cache reads
                                 + cusage.total_tool_use_tokens                                       // tools
                                 + cusage.total_output_tokens                                         // visible output
                                 + cusage.total_thought_tokens, // reasoning
    }
}

/// Recursively removes "additionalProperties" from the JSON schema to prevent
/// Gemini API validation errors, while keeping the source schema OpenAI-compliant.
pub fn sanitize_schema_for_gemini(value: &mut Value) {
    match value {
        Value::Object(map) => {
            // Remove the key at the current object level
            map.remove("additionalProperties");

            // Recurse down into properties, items, or any nested structures
            for (_, val) in map.iter_mut() {
                sanitize_schema_for_gemini(val);
            }
        }
        Value::Array(arr) => {
            // Recurse through arrays (like 'required' fields or array item definitions)
            for val in arr.iter_mut() {
                sanitize_schema_for_gemini(val);
            }
        }
        _ => {} // Base case: Strings, Numbers, Booleans, Nulls do nothing
    }
}
