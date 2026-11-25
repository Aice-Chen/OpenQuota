use crate::{models::LanguagePreference, pacing::Milestone};

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
    match label {
        "Session" => "会话",
        "Weekly" => "每周",
        "Extra Usage" => "额外用量",
        "Usage Trend" => "用量趋势",
        "Today" => "今日",
        "Yesterday" => "昨天",
        "Last 30 Days" => "近 30 天",
        "Rate Limit Resets" => "限额重置券",
        "Credits" => "点数",
        "Balance" => "余额",
        "This Week" => "本周",
        "This Month" => "本月",
        "Key Limit" => "密钥限额",
        "Requests" => "请求数",
        "Monthly" => "每月",
        "Daily" => "每日",
        _ => label,
    }
}

pub fn pace_alert_copy(
    milestone: Milestone,
    language: LanguagePreference,
) -> (&'static str, &'static str) {
    if is_zh(language) {
        match milestone {
            Milestone::AlmostOut => ("额度即将耗尽", "当前周期剩余额度已不足 10%。"),
            Milestone::CuttingItClose => ("进度偏紧", "按当前消耗速度，预计将接近限额。"),
            Milestone::WillRunOut => ("预计提前耗尽", "按当前消耗速度，预计会在重置前用完额度。"),
        }
    } else {
        (milestone.title(), milestone.body())
    }
}

#[cfg(test)]
mod tests {
    use super::{is_zh, pace_alert_copy, translate_metric_label};
    use crate::{models::LanguagePreference, pacing::Milestone};

    #[test]
    fn explicit_language_preferences_override_system_locale() {
        assert!(is_zh(LanguagePreference::ZhCn));
        assert!(!is_zh(LanguagePreference::En));
        assert_eq!(translate_metric_label("Session", true), "会话");
        assert_eq!(translate_metric_label("Session", false), "Session");
        let (zh_title, _) = pace_alert_copy(Milestone::AlmostOut, LanguagePreference::ZhCn);
        let (en_title, _) = pace_alert_copy(Milestone::AlmostOut, LanguagePreference::En);
        assert_eq!(zh_title, "额度即将耗尽");
        assert_eq!(en_title, "Almost Out");
    }
}
