pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const REVISION: &str = match option_env!("MODELPORT_BUILD_REVISION") {
    Some(value) => value,
    None => "unknown",
};
pub const SOURCE_STATE: &str = match option_env!("MODELPORT_BUILD_SOURCE_STATE") {
    Some(value) => value,
    None => "unknown",
};

pub fn display() -> String {
    format!("model-port {VERSION} (revision {REVISION}, source {SOURCE_STATE})")
}

pub fn json() -> serde_json::Value {
    serde_json::json!({
        "version": VERSION,
        "revision": REVISION,
        "sourceState": SOURCE_STATE,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_identity_is_always_present() {
        assert!(!VERSION.is_empty());
        assert!(!REVISION.is_empty());
        assert!(!SOURCE_STATE.is_empty());
        assert!(display().contains(VERSION));
    }
}
