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
