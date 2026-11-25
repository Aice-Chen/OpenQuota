use std::{path::Path, sync::OnceLock};

use serde::Deserialize;

const BUNDLED_RULES_JSON: &str = include_str!("../../../resources/antigravity_rules.json");

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AntigravityRules {
    pub hub_version: String,
    pub cloud_bases: Vec<String>,
    pub model_blacklist: Vec<String>,
    pub session_aliases: Vec<String>,
    pub weekly_aliases: Vec<String>,
    pub daily_aliases: Vec<String>,
    pub gemini_keywords: Vec<String>,
    pub claude_keywords: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct AntigravityRulesOverride {
    hub_version: Option<String>,
    cloud_bases: Option<Vec<String>>,
    model_blacklist: Option<Vec<String>>,
    session_aliases: Option<Vec<String>>,
    weekly_aliases: Option<Vec<String>>,
    daily_aliases: Option<Vec<String>>,
    gemini_keywords: Option<Vec<String>>,
    claude_keywords: Option<Vec<String>>,
}

impl AntigravityRules {
    pub fn bundled() -> &'static Self {
        static BUNDLED: OnceLock<AntigravityRules> = OnceLock::new();
        BUNDLED.get_or_init(|| {
            serde_json::from_str(BUNDLED_RULES_JSON)
                .expect("bundled antigravity_rules.json must be valid")
        })
    }

    pub fn load(override_path: Option<&Path>) -> Self {
        let mut rules = Self::bundled().clone();
        let Some(path) = override_path else {
            return rules;
        };
        let Ok(raw) = std::fs::read_to_string(path) else {
            return rules;
        };
        match serde_json::from_str::<AntigravityRulesOverride>(&raw) {
            Ok(custom) => {
                rules.apply_override(custom);
                crate::app_info!("config", "loaded custom Antigravity rules override");
            }
            Err(error) => {
                crate::app_warn!(
                    "config",
                    "ignoring invalid Antigravity rules override: {error}"
                );
            }
        }
        rules
    }

    pub fn hub_user_agent(&self) -> String {
        let version = self.hub_version.trim();
        let version = if version.is_empty() { "2.9.1" } else { version };
        let os = if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "darwin"
        } else {
            "linux"
        };
        let arch = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "amd64"
        };
        format!("antigravity/hub/{version} {os}/{arch}")
    }

    fn apply_override(&mut self, custom: AntigravityRulesOverride) {
        if let Some(version) = custom
            .hub_version
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
        {
            self.hub_version = version;
        }
        if let Some(bases) = custom.cloud_bases {
            let valid: Vec<String> = bases
                .into_iter()
                .map(|value| value.trim().trim_end_matches('/').to_owned())
                .filter(|value| value.starts_with("https://"))
                .collect();
            if !valid.is_empty() {
                self.cloud_bases = valid;
            }
        }
        if let Some(blacklist) = custom.model_blacklist {
            self.model_blacklist = clean_list(blacklist);
        }
        if let Some(aliases) = custom.session_aliases {
            let cleaned = clean_lower_list(aliases);
            if !cleaned.is_empty() {
                self.session_aliases = cleaned;
            }
        }
        if let Some(aliases) = custom.weekly_aliases {
            let cleaned = clean_lower_list(aliases);
            if !cleaned.is_empty() {
                self.weekly_aliases = cleaned;
            }
        }
        if let Some(aliases) = custom.daily_aliases {
            let cleaned = clean_lower_list(aliases);
            if !cleaned.is_empty() {
                self.daily_aliases = cleaned;
            }
        }
        if let Some(keywords) = custom.gemini_keywords {
            let cleaned = clean_lower_list(keywords);
            if !cleaned.is_empty() {
                self.gemini_keywords = cleaned;
            }
        }
        if let Some(keywords) = custom.claude_keywords {
            let cleaned = clean_lower_list(keywords);
            if !cleaned.is_empty() {
                self.claude_keywords = cleaned;
            }
        }
    }
}

fn clean_list(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect()
}

fn clean_lower_list(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::AntigravityRules;

    #[test]
    fn bundled_rules_load_and_build_platform_user_agent() {
        let rules = AntigravityRules::bundled();
        assert_eq!(rules.hub_version, "2.9.1");
        assert!(!rules.cloud_bases.is_empty());
        let has_gemini_25 = rules
            .model_blacklist
            .iter()
            .any(|id| id == "MODEL_GOOGLE_GEMINI_2_5_PRO");
        assert!(has_gemini_25);
        assert!(rules.hub_user_agent().starts_with("antigravity/hub/2.9.1 "));
    }

    #[test]
    fn local_override_merges_selected_fields_and_rejects_insecure_bases() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rules.json");
        std::fs::write(
            &path,
            r#"{
                "hubVersion": "3.1.0",
                "cloudBases": ["http://insecure.example", "https://custom.googleapis.com/"],
                "modelBlacklist": ["MODEL_CUSTOM_IGNORE"]
            }"#,
        )
        .unwrap();

        let rules = AntigravityRules::load(Some(&path));
        assert_eq!(rules.hub_version, "3.1.0");
        assert_eq!(rules.cloud_bases, vec!["https://custom.googleapis.com"]);
        assert_eq!(rules.model_blacklist, vec!["MODEL_CUSTOM_IGNORE"]);
        assert!(rules.session_aliases.iter().any(|alias| alias == "5h"));
    }
}
