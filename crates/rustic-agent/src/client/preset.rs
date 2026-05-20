// ── Preset ────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub enum Preset {
    Fast,     // speed first, low tokens, no cache
    Balanced, // good default for most tasks
    Precise,  // high reasoning, low temperature
    Thorough, // maximum quality, high tokens
    Local,    // optimised for local models (low tokens, no cache)
}
