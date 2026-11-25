use std::{collections::HashMap, sync::OnceLock};

use serde::Deserialize;

use crate::{models::LanguagePreference, pacing::Milestone};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SharedDictionary {
    metric_labels: HashMap<String, String>,
    units: HashMap<String, String>,
    tray_menu: LocalizedTrayMenu,
    tray_usage: LocalizedTrayUsage,
    pace_alerts: LocalizedPaceAlerts,
}

#[derive(Deserialize)]
struct LocalizedTrayMenu {
    en: TrayMenuLabels,
    #[serde(rename = "zh-CN")]
    zh_cn: TrayMenuLabels,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayMenuLabels {
    pub open: String,
    pub customize: String,
    pub settings: String,
    pub settings_mac: String,
    pub quit: String,
}

#[derive(Deserialize)]
struct LocalizedTrayUsage {
    en: TrayUsageWords,
    #[serde(rename = "zh-CN")]
    zh_cn: TrayUsageWords,
}

#[derive(Deserialize)]
struct TrayUsageWords {
    used: String,
    left: String,
}

#[derive(Deserialize)]
struct LocalizedPaceAlerts {
    #[serde(rename = "zh-CN")]
    zh_cn: PaceAlertSet,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PaceAlertSet {
    almost_out: PaceAlertEntry,
    cutting_it_close: PaceAlertEntry,
    will_run_out: PaceAlertEntry,
}

#[derive(Deserialize)]
struct PaceAlertEntry {
    title: String,
    body: String,
}

static SHARED: OnceLock<SharedDictionary> = OnceLock::new();

fn shared() -> &'static SharedDictionary {
    SHARED.get_or_init(|| {
        serde_json::from_str(include_str!("../../shared/i18n.json"))
            .expect("bundled shared/i18n.json must be valid")
    })
}

pub fn is_zh(language: LanguagePreference) -> bool {
    match language {
        LanguagePreference::ZhCn => true,
        LanguagePreference::En => false,
        LanguagePreference::System => system_is_zh(),
    }
}

fn system_is_zh() -> bool {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_ascii_lowercase().starts_with("zh");
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        windows_ui_language_is_zh()
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

#[cfg(target_os = "windows")]
fn windows_ui_language_is_zh() -> bool {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    // Primary language ID 0x04 corresponds to LANG_CHINESE.
    let lang_id = unsafe { GetUserDefaultUILanguage() };
    (lang_id & 0x03ff) == 0x04
}

pub fn translate_metric_label(label: &str, zh: bool) -> &str {
    if !zh {
        return label;
    }
    shared()
        .metric_labels
        .get(label)
        .map(String::as_str)
        .unwrap_or(label)
}

pub fn translate_unit(unit: &str, zh: bool) -> &str {
    if !zh {
        return unit;
    }
    let key = unit.trim().to_ascii_lowercase();
    shared()
        .units
        .get(&key)
        .map(String::as_str)
        .unwrap_or(unit)
}

pub fn tray_menu_labels(zh: bool) -> &'static TrayMenuLabels {
    let menu = &shared().tray_menu;
    if zh { &menu.zh_cn } else { &menu.en }
}

pub fn tray_usage_word(used: bool, zh: bool) -> &'static str {
    let words = if zh {
        &shared().tray_usage.zh_cn
    } else {
        &shared().tray_usage.en
    };
    if used {
        words.used.as_str()
    } else {
        words.left.as_str()
    }
}

pub fn pace_alert_copy(
    milestone: Milestone,
    language: LanguagePreference,
) -> (&'static str, &'static str) {
    if is_zh(language) {
        let alerts = &shared().pace_alerts.zh_cn;
        let entry = match milestone {
            Milestone::AlmostOut => &alerts.almost_out,
            Milestone::CuttingItClose => &alerts.cutting_it_close,
            Milestone::WillRunOut => &alerts.will_run_out,
        };
        (entry.title.as_str(), entry.body.as_str())
    } else {
        (milestone.title(), milestone.body())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        is_zh, pace_alert_copy, translate_metric_label, translate_unit, tray_menu_labels,
        tray_usage_word,
    };
    use crate::{models::LanguagePreference, pacing::Milestone};

    #[test]
    fn shared_dictionary_drives_locale_labels_and_alerts() {
        assert!(is_zh(LanguagePreference::ZhCn));
        assert!(!is_zh(LanguagePreference::En));
        assert_eq!(translate_metric_label("Session", true), "会话");
        assert_eq!(translate_metric_label("Web Searches", true), "联网搜索");
        assert_eq!(translate_metric_label("Session", false), "Session");
        assert_eq!(translate_unit("requests", true), "次请求");
        assert_eq!(translate_unit("requests", false), "requests");
        assert_eq!(tray_menu_labels(true).quit, "退出 OpenQuota");
        assert_eq!(tray_menu_labels(false).quit, "Quit OpenQuota");
        assert_eq!(tray_usage_word(true, true), "已用");
        assert_eq!(tray_usage_word(false, true), "剩余");
        assert_eq!(tray_usage_word(false, false), "left");
        let (zh_title, _) = pace_alert_copy(Milestone::AlmostOut, LanguagePreference::ZhCn);
        let (en_title, _) = pace_alert_copy(Milestone::AlmostOut, LanguagePreference::En);
        assert_eq!(zh_title, "额度即将耗尽");
        assert_eq!(en_title, "Almost Out");
    }
}
