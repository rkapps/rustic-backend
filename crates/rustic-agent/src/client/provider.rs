#[derive(Debug, Clone)]
pub enum Provider {
    OpenAI { api_key: String, model: String },
    Gemini { api_key: String, model: String },
    Anthropic { api_key: String, model: String },
    Groq { api_key: String, model: String },
    Together { api_key: String, model: String },
    Fireworks { api_key: String, model: String },
    Mistral { api_key: String, model: String },
    Local { model: String, base_url: String },
}

impl Provider {
    pub fn openai(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self::OpenAI {
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    pub fn gemini(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self::Gemini {
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    pub fn anthropic(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self::Anthropic {
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    pub fn groq(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self::Groq {
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    pub fn together(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self::Together {
            api_key: api_key.into(),
            model: model.into(),
        }
    }
    pub fn fireworks(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self::Fireworks {
            api_key: api_key.into(),
            model: model.into(),
        }
    }
    pub fn mistral(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self::Mistral {
            api_key: api_key.into(),
            model: model.into(),
        }
    }
    pub fn local(model: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self::Local {
            model: model.into(),
            base_url: base_url.into(),
        }
    }

    pub fn ollama(model: impl Into<String>) -> Self {
        Self::Local {
            model: model.into(),
            base_url: "http://localhost:11434".to_string(),
        }
    }
}
