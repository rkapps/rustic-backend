pub fn clean_content(s: &str) -> String {
    let s = s.trim();
    if s.starts_with("```json") {
        s.trim_start_matches("```json")
            .trim_end_matches("```")
            .trim()
            .to_string()
    } else if s.starts_with("```") {
        s.trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
            .to_string()
    } else {
        s.to_string()
    }
}

pub fn truncate(s: &str, max: usize) -> String {
    let s = clean_content(s);
    if s.len() <= max {
        return s;
    }
    // Snap down to the nearest char boundary at or below `max`
    let mut idx = max;
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    format!("{}...", &s[..idx])
}
