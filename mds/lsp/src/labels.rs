use mds_core::model::Config;
pub fn resolve_label(key: &str, config: &Config) -> String {
    if let Some(override_label) = config.label_overrides.get(key) {
        return override_label.clone();
    }
    if key.eq_ignore_ascii_case("api") {
        return "API".to_string();
    }
    // Default: capitalize first letter
    let mut chars = key.chars();
    match chars.next() {
        Some(first) => {
            let upper: String = first.to_uppercase().collect();
            format!("{upper}{}", chars.as_str())
        }
        None => key.to_string(),
    }
}
