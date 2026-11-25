use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::models::{QuotaFormat, QuotaWindow};

const MODEL_BLACKLIST: [&str; 9] = [
    "MODEL_CHAT_20706",
    "MODEL_CHAT_23310",
    "MODEL_GOOGLE_GEMINI_2_5_FLASH",
    "MODEL_GOOGLE_GEMINI_2_5_FLASH_THINKING",
    "MODEL_GOOGLE_GEMINI_2_5_FLASH_LITE",
    "MODEL_GOOGLE_GEMINI_2_5_PRO",
    "MODEL_PLACEHOLDER_M19",
    "MODEL_PLACEHOLDER_M9",
    "MODEL_PLACEHOLDER_M12",
];

#[derive(Debug)]
pub struct ModelConfig {
    label: String,
    model_id: Option<String>,
    remaining_fraction: f64,
    resets_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum QuotaSlot {
    GeminiSession,
    GeminiWeekly,
    ClaudeSession,
    ClaudeWeekly,
}

impl QuotaSlot {
    const ALL: [Self; 4] = [
        Self::GeminiSession,
        Self::GeminiWeekly,
        Self::ClaudeSession,
        Self::ClaudeWeekly,
    ];

    const fn metadata(self) -> (&'static str, &'static str, u64) {
        match self {
            Self::GeminiSession => ("geminiPro", "Session", 5 * 60 * 60),
            Self::GeminiWeekly => ("geminiWeekly", "Weekly", 7 * 24 * 60 * 60),
            Self::ClaudeSession => ("claude", "Claude", 5 * 60 * 60),
            Self::ClaudeWeekly => ("claudeWeekly", "Claude Weekly", 7 * 24 * 60 * 60),
        }
    }

    fn from_quota_id(id: &str) -> Option<Self> {
        match id {
            "geminiPro" => Some(Self::GeminiSession),
            "geminiWeekly" => Some(Self::GeminiWeekly),
            "claude" => Some(Self::ClaudeSession),
            "claudeWeekly" => Some(Self::ClaudeWeekly),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PoolFamily {
    Gemini { ordinary: bool },
    Claude,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuotaCadence {
    Session,
    Weekly,
}

struct BestBucket {
    priority: u8,
    remaining: f64,
    resets_at: Option<DateTime<Utc>>,
}

const SESSION_ALIASES: [&str; 6] = ["5h", "5hr", "5-hour", "five-hour", "five hour", "session"];
const WEEKLY_ALIASES: [&str; 5] = ["weekly", "week", "7d", "7-day", "7-days"];

pub fn parse_quota_summary(value: &Value) -> Option<Vec<QuotaWindow>> {
    let groups = value
        .pointer("/response/groups")
        .or_else(|| value.pointer("/summary/groups"))
        .or_else(|| value.get("groups"))?
        .as_array()?;
    let mut found: HashMap<QuotaSlot, BestBucket> = HashMap::new();
    for group in groups {
        let group_title = group
            .get("displayName")
            .or_else(|| group.get("name"))
            .and_then(Value::as_str)
            .and_then(trimmed);
        let Some(buckets) = group.get("buckets").and_then(Value::as_array) else {
            continue;
        };
        for bucket in buckets {
            if bucket.get("disabled").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            let Some((slot, priority)) = classify_summary_bucket(bucket, group_title) else {
                continue;
            };
            let Some(fraction) = extract_remaining_fraction(bucket) else {
                continue;
            };
            let reset = bucket
                .get("resetTime")
                .and_then(Value::as_str)
                .and_then(|text| DateTime::parse_from_rfc3339(text).ok())
                .map(|date| date.to_utc());
            let is_better = |b: &BestBucket| (priority, fraction) < (b.priority, b.remaining);
            if found.get(&slot).is_none_or(is_better) {
                found.insert(
                    slot,
                    BestBucket {
                        priority,
                        remaining: fraction,
                        resets_at: reset,
                    },
                );
            }
        }
    }
    let quotas: Vec<QuotaWindow> = QuotaSlot::ALL
        .into_iter()
        .filter_map(|slot| {
            let best = found.get(&slot)?;
            let (quota_id, label, period_seconds) = slot.metadata();
            Some(QuotaWindow {
                id: quota_id.into(),
                label: label.into(),
                used_percent: ((1.0 - best.remaining.clamp(0.0, 1.0)) * 100.0).round(),
                resets_at: best.resets_at,
                period_seconds,
                format: QuotaFormat::Percent,
                used_value: None,
                limit_value: None,
                unit: None,
                estimated: false,
                source_note: None,
            })
        })
        .collect();
    (!quotas.is_empty()).then_some(quotas)
}

pub fn merge_missing_quotas(
    primary: Vec<QuotaWindow>,
    fallback: Vec<QuotaWindow>,
) -> Vec<QuotaWindow> {
    let mut by_id = HashMap::new();
    let mut extra: Vec<QuotaWindow> = Vec::new();
    for quota in primary.into_iter().chain(fallback) {
        if let Some(slot) = QuotaSlot::from_quota_id(&quota.id) {
            by_id.entry(slot).or_insert(quota);
        } else if !extra.iter().any(|existing| existing.id == quota.id) {
            extra.push(quota);
        }
    }
    let mut ordered = QuotaSlot::ALL
        .into_iter()
        .filter_map(|slot| by_id.remove(&slot))
        .collect::<Vec<_>>();
    ordered.extend(extra);
    ordered
}

fn classify_summary_bucket(bucket: &Value, group_title: Option<&str>) -> Option<(QuotaSlot, u8)> {
    let bucket_id = bucket
        .get("bucketId")
        .or_else(|| bucket.get("id"))
        .and_then(Value::as_str)
        .and_then(trimmed);
    let display_name = bucket
        .get("displayName")
        .or_else(|| bucket.get("name"))
        .and_then(Value::as_str)
        .and_then(trimmed);
    let window = bucket
        .get("window")
        .and_then(Value::as_str)
        .and_then(trimmed);

    let cadence = detect_cadence(window, bucket_id, display_name)?;
    let family = detect_family(bucket_id, group_title, display_name)?;

    match (family, cadence) {
        (PoolFamily::Gemini { ordinary }, QuotaCadence::Session) => {
            Some((QuotaSlot::GeminiSession, u8::from(!ordinary)))
        }
        (PoolFamily::Gemini { ordinary }, QuotaCadence::Weekly) => {
            Some((QuotaSlot::GeminiWeekly, u8::from(!ordinary)))
        }
        (PoolFamily::Claude, QuotaCadence::Session) => Some((QuotaSlot::ClaudeSession, 0)),
        (PoolFamily::Claude, QuotaCadence::Weekly) => Some((QuotaSlot::ClaudeWeekly, 0)),
    }
}

fn detect_family(
    bucket_id: Option<&str>,
    group_title: Option<&str>,
    display_name: Option<&str>,
) -> Option<PoolFamily> {
    if let Some(id) = bucket_id {
        let normalized = normalize_token(id);
        if normalized.contains("gemini") {
            let ordinary = is_ordinary_gemini_bucket_id(&normalized);
            return Some(PoolFamily::Gemini { ordinary });
        }
        if normalized.contains("3p")
            || normalized.contains("third-party")
            || normalized.contains("claude")
            || normalized.contains("gpt")
        {
            return Some(PoolFamily::Claude);
        }
    }
    for text in [group_title, display_name].into_iter().flatten() {
        let lower = text.to_ascii_lowercase();
        if lower.contains("gemini") {
            let ordinary = !lower.chars().any(|ch| ch.is_ascii_digit());
            return Some(PoolFamily::Gemini { ordinary });
        }
        if lower.contains("claude")
            || lower.contains("gpt")
            || lower.contains("3p")
            || lower.contains("third-party")
        {
            return Some(PoolFamily::Claude);
        }
    }
    None
}

fn is_ordinary_gemini_bucket_id(normalized: &str) -> bool {
    let trimmed = normalized.strip_suffix(" limit").unwrap_or(normalized);
    SESSION_ALIASES
        .iter()
        .chain(WEEKLY_ALIASES.iter())
        .any(|alias| trimmed == format!("gemini-{alias}"))
        || trimmed == "gemini"
}

fn detect_cadence(
    window: Option<&str>,
    bucket_id: Option<&str>,
    display_name: Option<&str>,
) -> Option<QuotaCadence> {
    window
        .and_then(cadence_from_text)
        .or_else(|| bucket_id.and_then(cadence_from_text))
        .or_else(|| display_name.and_then(cadence_from_text))
}

fn cadence_from_text(raw: &str) -> Option<QuotaCadence> {
    let normalized = normalize_token(raw);
    let stripped = normalized.strip_suffix(" limit").unwrap_or(&normalized);
    if matches_alias(stripped, &SESSION_ALIASES) {
        return Some(QuotaCadence::Session);
    }
    if matches_alias(stripped, &WEEKLY_ALIASES) {
        return Some(QuotaCadence::Weekly);
    }
    None
}

fn matches_alias(value: &str, aliases: &[&str]) -> bool {
    aliases.iter().any(|alias| {
        value == *alias
            || value.ends_with(&format!("-{alias}"))
            || value.ends_with(&format!(" {alias}"))
            || value.starts_with(&format!("{alias}-"))
            || value.starts_with(&format!("{alias} "))
            || value.contains(&format!("-{alias}-"))
            || value.contains(&format!(" {alias} "))
    })
}

fn normalize_token(raw: &str) -> String {
    raw.trim().to_ascii_lowercase().replace('_', "-")
}

fn extract_remaining_fraction(bucket: &Value) -> Option<f64> {
    if let Some(fraction) = bucket.get("remainingFraction").and_then(number) {
        return Some(fraction);
    }
    let remaining = bucket.get("remaining")?;
    if let Some(fraction) = remaining.get("remainingFraction").and_then(number) {
        return Some(fraction);
    }
    if remaining.get("case").and_then(Value::as_str) == Some("remainingFraction") {
        return remaining.get("value").and_then(number);
    }
    None
}

pub fn parse_plan(value: &Value) -> Option<String> {
    let raw = value
        .pointer("/userStatus/userTier/name")
        .or_else(|| value.pointer("/userStatus/planStatus/planInfo/planName"))
        .and_then(Value::as_str)?
        .trim();
    if raw.is_empty() {
        return None;
    }
    if let Some(value) = raw.strip_prefix("Google AI ") {
        return Some(title_case(value));
    }
    for tier in ["Ultra", "Pro", "Free"] {
        if raw
            .to_ascii_lowercase()
            .contains(&tier.to_ascii_lowercase())
        {
            return Some(tier.into());
        }
    }
    Some(title_case(raw))
}

pub fn parse_user_status(value: &Value) -> Option<(Option<String>, Vec<ModelConfig>)> {
    value.get("userStatus")?;
    let configs = value
        .pointer("/userStatus/cascadeModelConfigData/clientModelConfigs")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(model_from_ls).collect())
        .unwrap_or_default();
    Some((parse_plan(value), configs))
}

pub fn parse_command_model_configs(value: &Value) -> Option<Vec<ModelConfig>> {
    Some(
        value
            .get("clientModelConfigs")?
            .as_array()?
            .iter()
            .filter_map(model_from_ls)
            .collect(),
    )
}

pub fn parse_cloud_models(value: &Value) -> Vec<ModelConfig> {
    let Some(models) = value.get("models").and_then(Value::as_object) else {
        return Vec::new();
    };
    models
        .iter()
        .filter_map(|(key, model)| {
            if model.get("isInternal").and_then(Value::as_bool) == Some(true) {
                return None;
            }
            let label = model
                .get("displayName")
                .and_then(Value::as_str)
                .and_then(trimmed)
                .or_else(|| model.get("label").and_then(Value::as_str).and_then(trimmed))?;
            let model_id = model
                .get("model")
                .and_then(Value::as_str)
                .and_then(trimmed)
                .unwrap_or(key)
                .to_owned();
            Some(model_config(label, Some(model_id), model.get("quotaInfo")))
        })
        .collect()
}

pub fn parse_quota_buckets(value: &Value) -> Vec<ModelConfig> {
    value
        .get("buckets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|bucket| {
            let id = bucket
                .get("modelId")
                .and_then(Value::as_str)
                .and_then(trimmed)?
                .to_owned();
            Some(model_config(&id, Some(id.clone()), Some(bucket)))
        })
        .collect()
}

pub fn build_legacy_quotas(configs: Vec<ModelConfig>) -> Vec<QuotaWindow> {
    let mut gemini: Option<(f64, Option<chrono::DateTime<chrono::Utc>>)> = None;
    let mut claude: Option<(f64, Option<chrono::DateTime<chrono::Utc>>)> = None;
    for config in configs {
        if config
            .model_id
            .as_deref()
            .is_some_and(|id| MODEL_BLACKLIST.contains(&id))
        {
            continue;
        }
        let is_gemini = config.label.to_ascii_lowercase().contains("gemini")
            || config
                .model_id
                .as_deref()
                .is_some_and(|id| id.to_ascii_lowercase().contains("gemini"));
        let pool = if is_gemini {
            &mut gemini
        } else {
            &mut claude
        };
        if pool
            .as_ref()
            .is_none_or(|(remaining, _)| config.remaining_fraction < *remaining)
        {
            *pool = Some((config.remaining_fraction, config.resets_at));
        }
    }
    [
        ("geminiPro", "Session", gemini),
        ("claude", "Claude", claude),
    ]
    .into_iter()
    .filter_map(|(id, label, pool)| {
        let (remaining, resets_at) = pool?;
        Some(QuotaWindow {
            id: id.into(),
            label: label.into(),
            used_percent: ((1.0 - remaining.clamp(0.0, 1.0)) * 100.0).round(),
            resets_at,
            period_seconds: 5 * 60 * 60,
            format: QuotaFormat::Percent,
            used_value: None,
            limit_value: None,
            unit: None,
            estimated: false,
            source_note: None,
        })
    })
    .collect()
}

fn model_from_ls(value: &Value) -> Option<ModelConfig> {
    let label = value
        .get("label")
        .and_then(Value::as_str)
        .and_then(trimmed)?;
    let model_id = value
        .pointer("/modelOrAlias/model")
        .and_then(Value::as_str)
        .and_then(trimmed)
        .map(str::to_owned);
    Some(model_config(label, model_id, value.get("quotaInfo")))
}

fn model_config(label: &str, model_id: Option<String>, quota: Option<&Value>) -> ModelConfig {
    ModelConfig {
        label: label.to_owned(),
        model_id,
        remaining_fraction: quota
            .and_then(|value| value.get("remainingFraction"))
            .and_then(number)
            .unwrap_or(0.0),
        resets_at: quota
            .and_then(|value| value.get("resetTime"))
            .and_then(Value::as_str)
            .and_then(|text| DateTime::parse_from_rfc3339(text).ok())
            .map(|date| date.to_utc()),
    }
}

fn trimmed(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn title_case(value: &str) -> String {
    value
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        .filter(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{
        build_legacy_quotas, merge_missing_quotas, parse_cloud_models, parse_command_model_configs,
        parse_quota_buckets, parse_quota_summary, parse_user_status,
    };

    #[test]
    fn maps_authoritative_pool_summary_in_fixed_order() {
        let value: Value = serde_json::from_str(include_str!("fixtures/quota_summary.json"))
            .expect("valid fixture");
        let quotas = parse_quota_summary(&value).unwrap();
        assert_eq!(quotas.len(), 2);
        assert_eq!(quotas[0].id, "geminiPro");
        assert_eq!(quotas[0].used_percent, 20.0);
        assert_eq!(quotas[1].id, "claudeWeekly");
    }

    #[test]
    fn empty_or_unknown_quota_summary_returns_none_for_fallback() {
        let value = json!({
            "response": {
                "groups": [
                    {
                        "buckets": [
                            { "bucketId": "unknown", "remainingFraction": 1.0 },
                            { "bucketId": "gemini-5h", "remainingFraction": 0.5, "disabled": true }
                        ]
                    }
                ]
            }
        });
        assert!(parse_quota_summary(&value).is_none());
    }

    #[test]
    fn parses_nested_remaining_shapes_cadence_aliases_and_prefers_ordinary_gemini() {
        let value = json!({
            "summary": {
                "groups": [
                    {
                        "displayName": "Gemini 4 Argon",
                        "buckets": [
                            {
                                "bucketId": "gemini-argon-wtu-weekly",
                                "displayName": "Weekly Limit",
                                "remaining": { "case": "remainingFraction", "value": 0.25 }
                            }
                        ]
                    },
                    {
                        "displayName": "Gemini Models",
                        "buckets": [
                            {
                                "id": "gemini_session",
                                "displayName": "5-hour Limit",
                                "remaining": { "remainingFraction": 0.75 }
                            },
                            {
                                "bucketId": "gemini-weekly",
                                "displayName": "Weekly Limit",
                                "remaining": { "remainingFraction": 0.6 }
                            }
                        ]
                    },
                    {
                        "displayName": "Claude and GPT models",
                        "buckets": [
                            {
                                "bucketId": "third-party-5-hour",
                                "window": "5h",
                                "remainingFraction": 0.9
                            }
                        ]
                    }
                ]
            }
        });
        let quotas = parse_quota_summary(&value).unwrap();
        assert_eq!(quotas.len(), 3);
        assert_eq!(quotas[0].id, "geminiPro");
        assert_eq!(quotas[0].used_percent, 25.0);
        assert_eq!(quotas[1].id, "geminiWeekly");
        assert_eq!(quotas[1].used_percent, 40.0);
        assert_eq!(quotas[2].id, "claude");
        assert_eq!(quotas[2].used_percent, 10.0);
    }

    #[test]
    fn falls_back_to_versioned_gemini_and_merges_missing_buckets() {
        let summary = json!({
            "groups": [
                {
                    "displayName": "Gemini 4 Argon",
                    "buckets": [
                        {
                            "bucketId": "gemini-argon-wtu-weekly",
                            "remainingFraction": 0.7
                        }
                    ]
                },
                {
                    "displayName": "Claude and GPT models",
                    "buckets": [
                        { "bucketId": "3p-5h", "remainingFraction": 0.8 }
                    ]
                }
            ]
        });
        let primary = parse_quota_summary(&summary).unwrap();
        let fallback = build_legacy_quotas(parse_quota_buckets(&json!({
            "buckets": [
                { "modelId": "gemini-3-pro", "remainingFraction": 0.6 },
                { "modelId": "claude-sonnet", "remainingFraction": 0.1 }
            ]
        })));
        let merged = merge_missing_quotas(primary, fallback);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].id, "geminiPro");
        assert_eq!(merged[0].used_percent, 40.0);
        assert_eq!(merged[1].id, "geminiWeekly");
        assert_eq!(merged[1].used_percent, 30.0);
        assert_eq!(merged[2].id, "claude");
        assert_eq!(merged[2].used_percent, 20.0);
    }

    #[test]
    fn pools_language_server_models_by_the_worst_remaining_fraction() {
        let value = json!({"userStatus": {
            "userTier": {"name": "Google AI Pro"},
            "cascadeModelConfigData": {"clientModelConfigs": [
                {"label": "Gemini 3 Pro", "quotaInfo": {"remainingFraction": 0.5}},
                {"label": "Gemini Flash", "quotaInfo": {"remainingFraction": 0.8}},
                {"label": "Claude Sonnet", "quotaInfo": {"remainingFraction": 0.7}}
            ]}
        }});
        let (plan, configs) = parse_user_status(&value).unwrap();
        let quotas = build_legacy_quotas(configs);
        assert_eq!(plan.as_deref(), Some("Pro"));
        assert_eq!(
            quotas
                .iter()
                .map(|quota| quota.id.as_str())
                .collect::<Vec<_>>(),
            ["geminiPro", "claude"]
        );
        assert_eq!(quotas[0].used_percent, 50.0);
        assert_eq!(quotas[1].used_percent, 30.0);
    }

    #[test]
    fn command_models_cloud_models_and_quota_buckets_share_pooling_rules() {
        let command = json!({"clientModelConfigs": [
            {"label": "Claude Opus", "quotaInfo": {"remainingFraction": 0.25}}
        ]});
        assert_eq!(
            build_legacy_quotas(parse_command_model_configs(&command).unwrap())[0].id,
            "claude"
        );

        let cloud = json!({"models": {
            "good": {"model": "MODEL_GOOD", "displayName": "Gemini 3 Pro", "quotaInfo": {"remainingFraction": 0.4}},
            "internal": {"displayName": "Claude Internal", "isInternal": true},
            "blacklisted": {"model": "MODEL_GOOGLE_GEMINI_2_5_PRO", "displayName": "Gemini 2.5 Pro"}
        }});
        let quotas = build_legacy_quotas(parse_cloud_models(&cloud));
        assert_eq!(quotas.len(), 1);
        assert_eq!(quotas[0].used_percent, 60.0);

        let buckets = json!({"buckets": [
            {"modelId": "gemini-3-pro", "remainingFraction": 0.5},
            {"modelId": "gemini-3-flash", "remainingFraction": 0.9}
        ]});
        let quotas = build_legacy_quotas(parse_quota_buckets(&buckets));
        assert_eq!(quotas[0].id, "geminiPro");
        assert_eq!(quotas[0].used_percent, 50.0);
    }
}
