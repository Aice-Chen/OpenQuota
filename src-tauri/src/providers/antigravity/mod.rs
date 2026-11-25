mod auth;
mod client;
mod discovery;
mod mapper;
mod rules;

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use chrono::Utc;
use serde_json::{json, Value};
use thiserror::Error;

use crate::{
    hashing::sha256_hex,
    models::{MetricDefinition, MetricSection, ProviderDefinition, ProviderSnapshot, UsageHistory},
    providers::{is_account_provider_id, CacheIdentity, UsageProvider},
    storage::Storage,
};

use self::{
    auth::{
        delete_account_credential, extract_token, has_account_credentials, load_account_token,
        load_token, normalize_manual_credential, read_default_raw_credential,
        save_account_credential, token_identity, AccessTokenCache,
    },
    client::{AntigravityClient, CloudOutcome, CloudUserAgent, RefreshOutcome},
    discovery::discover,
    mapper::{
        build_legacy_quotas_with_rules, merge_missing_quotas, parse_cloud_models,
        parse_command_model_configs, parse_plan, parse_quota_buckets,
        parse_quota_summary_with_rules, parse_user_status,
    },
    rules::AntigravityRules,
};

const FAMILY: &str = "antigravity";
const QUOTA_SUMMARY_PATH: &str = "/v1internal:retrieveUserQuotaSummary";
const FETCH_MODELS_PATH: &str = "/v1internal:fetchAvailableModels";
const LOAD_CODE_ASSIST_PATH: &str = "/v1internal:loadCodeAssist";
const RETRIEVE_QUOTA_PATH: &str = "/v1internal:retrieveUserQuota";

pub(crate) fn definition() -> ProviderDefinition {
    definition_for(FAMILY, "Antigravity")
}

pub(crate) fn definition_for(provider_id: &str, display_name: &str) -> ProviderDefinition {
    ProviderDefinition {
        id: provider_id.into(),
        display_name: display_name.into(),
        short_name: "A".into(),
        fallback_enabled: false,
        dynamic_quotas: true,
        local_usage_source_note: None,
        links: vec![],
        metrics: vec![
            MetricDefinition::quota(
                &format!("{provider_id}.geminiPro"),
                "Session",
                "geminiPro",
                true,
                true,
                MetricSection::AlwaysVisible,
                true,
                "S",
            ),
            MetricDefinition::quota(
                &format!("{provider_id}.geminiWeekly"),
                "Weekly",
                "geminiWeekly",
                false,
                true,
                MetricSection::AlwaysVisible,
                true,
                "W",
            ),
            MetricDefinition::quota(
                &format!("{provider_id}.claude"),
                "Claude",
                "claude",
                true,
                true,
                MetricSection::OnDemand,
                false,
                "C",
            ),
            MetricDefinition::quota(
                &format!("{provider_id}.claudeWeekly"),
                "Claude Weekly",
                "claudeWeekly",
                false,
                true,
                MetricSection::OnDemand,
                false,
                "CW",
            ),
        ],
    }
}

#[derive(Debug, Error)]
pub enum AntigravityError {
    #[error("Start Antigravity or run `agy` and try again.")]
    NotSignedIn,
    #[error("Antigravity sign-in expired. Open Antigravity or run `agy` to refresh.")]
    AuthExpired,
    #[error("Antigravity credentials could not be read from secure storage.")]
    CredentialStoreUnreadable,
    #[error("Antigravity credentials are invalid. Sign in again in Antigravity or `agy`.")]
    InvalidCredentialData,
    #[error("Antigravity usage is temporarily unavailable. Try again shortly.")]
    Unavailable,
}

enum AntigravityAccountMode {
    Default,
    SavedAccount {
        provider_id: String,
        display_name: String,
        identity: String,
    },
}

pub struct AddedAntigravityAccount {
    pub provider_id: String,
    pub custom_name: Option<String>,
    pub runtime: Arc<dyn UsageProvider>,
}

pub struct AntigravityProvider {
    account: AntigravityAccountMode,
    client: AntigravityClient,
    access_token_cache: AccessTokenCache,
    rules: AntigravityRules,
}

impl AntigravityProvider {
    pub fn new(cache_path: PathBuf) -> Result<Self, AntigravityError> {
        let rules_path = cache_path.parent().map(|dir| dir.join("rules.json"));
        let rules = AntigravityRules::load(rules_path.as_deref());
        Ok(Self {
            account: AntigravityAccountMode::Default,
            client: AntigravityClient::with_rules(&rules)?,
            access_token_cache: AccessTokenCache::new(cache_path),
            rules,
        })
    }

    pub fn for_saved_account(
        app_data_dir: &Path,
        provider_id: String,
        display_name: String,
        identity: String,
    ) -> Result<Self, AntigravityError> {
        let dir = app_data_dir.join(FAMILY);
        let rules_path = dir.join("rules.json");
        let rules = AntigravityRules::load(Some(&rules_path));
        let cache_path = account_cache_path(app_data_dir, &provider_id);
        Ok(Self {
            account: AntigravityAccountMode::SavedAccount {
                provider_id,
                display_name,
                identity,
            },
            client: AntigravityClient::with_rules(&rules)?,
            access_token_cache: AccessTokenCache::new(cache_path),
            rules,
        })
    }

    fn provider_id(&self) -> &str {
        match &self.account {
            AntigravityAccountMode::Default => FAMILY,
            AntigravityAccountMode::SavedAccount { provider_id, .. } => provider_id.as_str(),
        }
    }

    fn load_keychain_token(&self) -> Result<Option<auth::AntigravityToken>, AntigravityError> {
        match &self.account {
            AntigravityAccountMode::Default => load_token(),
            AntigravityAccountMode::SavedAccount { provider_id, .. } => {
                load_account_token(provider_id)
            }
        }
    }

    fn refresh_inner(&self) -> Result<ProviderSnapshot, AntigravityError> {
        if matches!(self.account, AntigravityAccountMode::Default) {
            if let Some(server) = discover() {
                let summary_quotas = self
                    .client
                    .call_language_server(&server, "RetrieveUserQuotaSummary")
                    .as_ref()
                    .and_then(|value| parse_quota_summary_with_rules(value, &self.rules));

                if let Some(mut quotas) = summary_quotas {
                    let status = self.client.call_language_server(&server, "GetUserStatus");
                    let plan = status.as_ref().and_then(parse_plan);
                    if !has_gemini_session(&quotas) {
                        if let Some((_, configs)) = status.as_ref().and_then(parse_user_status) {
                            quotas = merge_missing_quotas(
                                quotas,
                                build_legacy_quotas_with_rules(configs, &self.rules),
                            );
                        }
                        if !has_gemini_session(&quotas) {
                            if let Some(configs) = self
                                .client
                                .call_language_server(&server, "GetCommandModelConfigs")
                                .as_ref()
                                .and_then(parse_command_model_configs)
                            {
                                quotas = merge_missing_quotas(
                                    quotas,
                                    build_legacy_quotas_with_rules(configs, &self.rules),
                                );
                            }
                        }
                    }
                    return Ok(snapshot(self.provider_id(), plan, quotas));
                }

                if let Some(status) = self.client.call_language_server(&server, "GetUserStatus") {
                    if let Some((plan, configs)) = parse_user_status(&status) {
                        let quotas = build_legacy_quotas_with_rules(configs, &self.rules);
                        if !quotas.is_empty() {
                            return Ok(snapshot(self.provider_id(), plan, quotas));
                        }
                    }
                    if let Some(configs) = self
                        .client
                        .call_language_server(&server, "GetCommandModelConfigs")
                        .as_ref()
                        .and_then(parse_command_model_configs)
                    {
                        let quotas = build_legacy_quotas_with_rules(configs, &self.rules);
                        if !quotas.is_empty() {
                            return Ok(snapshot(self.provider_id(), None, quotas));
                        }
                    }
                }
            }
        }

        let keychain = match self.load_keychain_token()? {
            Some(token) => token,
            None => {
                self.access_token_cache.discard();
                return Err(AntigravityError::NotSignedIn);
            }
        };
        let now = Utc::now();
        let cached = self
            .access_token_cache
            .load(keychain.refresh_token.as_deref(), now);
        let (access_tokens, access_is_expired) = access_token_candidates(&keychain, cached, now);

        let mut saw_auth_failure = access_is_expired;
        let mut saw_unavailable = false;
        for access_token in &access_tokens {
            match self.fetch_remote(access_token) {
                Ok(snapshot) => return Ok(snapshot),
                Err(AntigravityError::AuthExpired) => saw_auth_failure = true,
                Err(AntigravityError::Unavailable) => saw_unavailable = true,
                Err(error) => return Err(error),
            }
        }

        let refresh_token = keychain
            .refresh_token
            .as_deref()
            .map(str::trim)
            .filter(|token| !token.is_empty());
        if let Some(refresh_token) = refresh_token.filter(|_| {
            should_refresh_access_token(saw_auth_failure, !access_tokens.is_empty(), true)
        }) {
            match self.client.refresh_google_token(refresh_token) {
                RefreshOutcome::Refreshed {
                    access_token,
                    expires_in_seconds,
                } => {
                    crate::app_info!("auth:antigravity", "token refresh succeeded");
                    self.access_token_cache.store(
                        &access_token,
                        expires_in_seconds,
                        Some(refresh_token),
                        Utc::now(),
                    );
                    return match self.fetch_remote(&access_token) {
                        Err(AntigravityError::AuthExpired) => {
                            self.access_token_cache.discard();
                            Err(AntigravityError::AuthExpired)
                        }
                        result => result,
                    };
                }
                RefreshOutcome::AuthFailed => {
                    self.access_token_cache.discard();
                    return Err(AntigravityError::AuthExpired);
                }
                RefreshOutcome::Unavailable => return Err(AntigravityError::Unavailable),
            }
        }
        Err(credential_failure(
            saw_auth_failure,
            saw_unavailable,
            !access_tokens.is_empty(),
        ))
    }

    fn fetch_remote(&self, token: &str) -> Result<ProviderSnapshot, AntigravityError> {
        let (plan, project) = self.load_code_assist(token)?;
        let project = project.as_deref();

        let mut summary_quotas: Option<Vec<crate::models::QuotaWindow>> = None;
        match self.cloud_code_with_project(
            QUOTA_SUMMARY_PATH,
            token,
            project,
            CloudUserAgent::Antigravity,
        ) {
            CloudOutcome::Ok(value) => {
                if let Some(quotas) = parse_quota_summary_with_rules(&value, &self.rules) {
                    if has_gemini_session(&quotas) {
                        return Ok(snapshot(self.provider_id(), plan, quotas));
                    }
                    summary_quotas = Some(quotas);
                }
            }
            CloudOutcome::AuthFailed => return Err(AntigravityError::AuthExpired),
            CloudOutcome::Unavailable => {}
        }

        match self.cloud_code_with_project(
            FETCH_MODELS_PATH,
            token,
            project,
            CloudUserAgent::Antigravity,
        ) {
            CloudOutcome::Ok(value) => {
                let fallback =
                    build_legacy_quotas_with_rules(parse_cloud_models(&value), &self.rules);
                if !fallback.is_empty() {
                    let merged = match summary_quotas {
                        Some(primary) => merge_missing_quotas(primary, fallback),
                        None => fallback,
                    };
                    return Ok(snapshot(self.provider_id(), plan, merged));
                }
            }
            CloudOutcome::AuthFailed => return Err(AntigravityError::AuthExpired),
            CloudOutcome::Unavailable => {}
        }

        match self.cloud_code_with_project(RETRIEVE_QUOTA_PATH, token, project, CloudUserAgent::Agy)
        {
            CloudOutcome::Ok(value) => {
                let fallback =
                    build_legacy_quotas_with_rules(parse_quota_buckets(&value), &self.rules);
                if !fallback.is_empty() {
                    let merged = match summary_quotas {
                        Some(primary) => merge_missing_quotas(primary, fallback),
                        None => fallback,
                    };
                    return Ok(snapshot(self.provider_id(), plan, merged));
                }
            }
            CloudOutcome::AuthFailed => return Err(AntigravityError::AuthExpired),
            CloudOutcome::Unavailable => {}
        }
        if let Some(quotas) = summary_quotas {
            return Ok(snapshot(self.provider_id(), plan, quotas));
        }
        Err(AntigravityError::Unavailable)
    }

    fn cloud_code_with_project(
        &self,
        path: &str,
        token: &str,
        project: Option<&str>,
        user_agent: CloudUserAgent,
    ) -> CloudOutcome {
        let body = project
            .map(|project| json!({"project": project}))
            .unwrap_or_else(|| json!({}));
        let outcome = self.client.cloud_code(path, token, body, user_agent);
        if matches!(outcome, CloudOutcome::Unavailable) && project.is_some() {
            return self.client.cloud_code(path, token, json!({}), user_agent);
        }
        outcome
    }

    fn load_code_assist(
        &self,
        token: &str,
    ) -> Result<(Option<String>, Option<String>), AntigravityError> {
        let metadata_body = json!({
            "metadata": {
                "ideType": "ANTIGRAVITY",
                "platform": "PLATFORM_UNSPECIFIED",
                "pluginType": "GEMINI"
            }
        });
        let primary = self.client.cloud_code(
            LOAD_CODE_ASSIST_PATH,
            token,
            metadata_body,
            CloudUserAgent::Antigravity,
        );
        let outcome = match primary {
            CloudOutcome::Unavailable => {
                self.client
                    .cloud_code(LOAD_CODE_ASSIST_PATH, token, json!({}), CloudUserAgent::Agy)
            }
            other => other,
        };
        match outcome {
            CloudOutcome::Ok(value) => Ok((
                remote_plan(&value),
                value
                    .get("cloudaicompanionProject")
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
                    .map(str::to_owned),
            )),
            CloudOutcome::AuthFailed => Err(AntigravityError::AuthExpired),
            CloudOutcome::Unavailable => Ok((None, None)),
        }
    }
}

fn has_gemini_session(quotas: &[crate::models::QuotaWindow]) -> bool {
    quotas.iter().any(|quota| quota.id == "geminiPro")
}

fn access_token_candidates(
    source: &auth::AntigravityToken,
    cached: Option<String>,
    now: chrono::DateTime<Utc>,
) -> (Vec<String>, bool) {
    let access_is_expired = source.access_token.as_ref().is_some_and(|_| {
        source
            .expiry
            .is_some_and(|expiry| expiry <= now + chrono::Duration::seconds(60))
    });
    let mut candidates = Vec::new();
    if !access_is_expired {
        if let Some(access_token) = source
            .access_token
            .as_deref()
            .map(str::trim)
            .filter(|token| !token.is_empty())
        {
            candidates.push(access_token.to_owned());
        }
    }
    if let Some(cached) = cached.filter(|token| !token.trim().is_empty()) {
        if !candidates.iter().any(|token| token == &cached) {
            candidates.push(cached);
        }
    }
    (candidates, access_is_expired)
}

fn should_refresh_access_token(
    saw_auth_failure: bool,
    has_access_candidate: bool,
    has_refresh_token: bool,
) -> bool {
    has_refresh_token && (saw_auth_failure || !has_access_candidate)
}

fn credential_failure(
    saw_auth_failure: bool,
    saw_unavailable: bool,
    tried_access_token: bool,
) -> AntigravityError {
    if saw_auth_failure {
        AntigravityError::AuthExpired
    } else if saw_unavailable || tried_access_token {
        AntigravityError::Unavailable
    } else {
        AntigravityError::NotSignedIn
    }
}

fn remote_plan(value: &Value) -> Option<String> {
    let raw = value
        .pointer("/paidTier/name")
        .or_else(|| value.pointer("/currentTier/name"))
        .and_then(Value::as_str)?;
    for tier in ["Ultra", "Pro", "Free"] {
        if raw
            .to_ascii_lowercase()
            .contains(&tier.to_ascii_lowercase())
        {
            return Some(tier.into());
        }
    }
    Some(raw.into())
}

fn snapshot(
    provider_id: &str,
    plan: Option<String>,
    quotas: Vec<crate::models::QuotaWindow>,
) -> ProviderSnapshot {
    ProviderSnapshot {
        provider_id: provider_id.into(),
        plan,
        quotas,
        value_metrics: Vec::new(),
        status_metrics: Vec::new(),
        notices: Vec::new(),
        usage: UsageHistory::default(),
        warnings: Vec::new(),
        refreshed_at: Utc::now(),
    }
}

fn account_cache_path(app_data_dir: &Path, provider_id: &str) -> PathBuf {
    let suffix = provider_id
        .split_once('@')
        .map(|(_, suffix)| suffix)
        .unwrap_or(provider_id);
    app_data_dir
        .join(FAMILY)
        .join(format!("auth_{suffix}.json"))
}

fn allocate_account_provider_id(
    records: &[(String, String, String)],
    existing_ids: &HashSet<String>,
    identity: &str,
) -> String {
    let mut occupied = records
        .iter()
        .map(|(_, provider_id, _)| provider_id.as_str())
        .collect::<HashSet<_>>();
    for id in existing_ids {
        occupied.insert(id.as_str());
    }
    for salt in 0_u32.. {
        let suffix = if salt == 0
            && identity.len() >= 8
            && identity.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            identity[..8].to_ascii_lowercase()
        } else {
            sha256_hex(format!("{identity}:{salt}").as_bytes())[..8].to_owned()
        };
        let candidate = format!("{FAMILY}@{suffix}");
        if !occupied.contains(candidate.as_str()) {
            return candidate;
        }
    }
    unreachable!()
}

pub fn saved_account_runtimes(
    storage: &Storage,
    app_data_dir: &Path,
) -> Result<Vec<Arc<dyn UsageProvider>>, AntigravityError> {
    let records = match storage.load_provider_account_records(FAMILY) {
        Ok(records) => records,
        Err(error) => {
            crate::app_warn!(
                "config",
                "saved Antigravity account records could not be loaded: {error}"
            );
            return Ok(Vec::new());
        }
    };
    let mut runtimes = Vec::new();
    for (identity, provider_id, payload) in records {
        if !provider_id.starts_with("antigravity@") || !is_account_provider_id(&provider_id) {
            continue;
        }
        let display_name = serde_json::from_str::<Value>(&payload)
            .ok()
            .and_then(|value| {
                value
                    .get("displayName")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "Antigravity".to_owned());
        let provider = AntigravityProvider::for_saved_account(
            app_data_dir,
            provider_id,
            display_name,
            identity,
        )?;
        runtimes.push(Arc::new(provider) as Arc<dyn UsageProvider>);
    }
    Ok(runtimes)
}

pub fn create_account(
    storage: &Storage,
    app_data_dir: &Path,
    existing_ids: &HashSet<String>,
    label: Option<&str>,
    raw_credential: Option<&str>,
) -> Result<AddedAntigravityAccount, String> {
    create_account_with_store(
        storage,
        app_data_dir,
        existing_ids,
        label,
        raw_credential,
        read_default_raw_credential,
        save_account_credential,
        delete_account_credential,
    )
}

#[allow(clippy::too_many_arguments)]
fn create_account_with_store<FRead, FSave, FDelete>(
    storage: &Storage,
    app_data_dir: &Path,
    existing_ids: &HashSet<String>,
    label: Option<&str>,
    raw_credential: Option<&str>,
    read_default: FRead,
    save_secret: FSave,
    delete_secret: FDelete,
) -> Result<AddedAntigravityAccount, String>
where
    FRead: FnOnce() -> Result<Option<Vec<u8>>, AntigravityError>,
    FSave: FnOnce(&str, &[u8]) -> Result<(), String>,
    FDelete: FnOnce(&str) -> Result<(), String>,
{
    let trimmed_input = raw_credential
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let (token, raw_bytes) = if let Some(input) = trimmed_input {
        normalize_manual_credential(input).ok_or_else(|| {
            "The pasted credential is invalid. Paste a Google refresh token (1//...) or valid Antigravity credential JSON.".to_owned()
        })?
    } else {
        let raw = read_default()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| {
                "No saved Antigravity sign-in was found in secure storage. Run `agy` to sign in, or paste a refresh token / credential JSON.".to_owned()
            })?;
        let token = extract_token(&raw)
            .ok_or_else(|| AntigravityError::InvalidCredentialData.to_string())?;
        (token, raw)
    };

    let identity = token_identity(&token).ok_or_else(|| {
        "The pasted credential is invalid. Paste a Google refresh token (1//...) or valid Antigravity credential JSON.".to_owned()
    })?;
    let records = storage
        .load_provider_account_records(FAMILY)
        .map_err(|_| "Account metadata could not be saved.".to_owned())?;
    if records.iter().any(|(known_identity, provider_id, _)| {
        known_identity == &identity && is_account_provider_id(provider_id)
    }) {
        return Err(
            "That account is already added. Sign in to another account first or paste different credentials.".to_owned(),
        );
    }

    let provider_id = allocate_account_provider_id(&records, existing_ids, &identity);
    let custom_name = label
        .map(|value| value.trim().chars().take(48).collect::<String>())
        .filter(|value| !value.is_empty());
    let default_display_name = "Antigravity".to_owned();
    let payload = match &custom_name {
        Some(name) => json!({
            "displayName": default_display_name,
            "customName": name,
        }),
        None => json!({
            "displayName": default_display_name,
        }),
    };
    let payload_text = serde_json::to_string(&payload)
        .map_err(|_| "Account metadata could not be saved.".to_owned())?;

    save_secret(&provider_id, &raw_bytes)?;
    if let Err(error) = storage
        .save_provider_account_record(FAMILY, &identity, &provider_id, &payload_text)
        .map_err(|_| "Account metadata could not be saved.".to_owned())
    {
        let _ = delete_secret(&provider_id);
        return Err(error);
    }

    let runtime = AntigravityProvider::for_saved_account(
        app_data_dir,
        provider_id.clone(),
        default_display_name,
        identity,
    )
    .map_err(|error| {
        let _ = storage.delete_provider_account_record(FAMILY, &provider_id);
        let _ = delete_secret(&provider_id);
        error.to_string()
    })?;

    Ok(AddedAntigravityAccount {
        provider_id,
        custom_name,
        runtime: Arc::new(runtime),
    })
}

pub fn remove_account(
    storage: &Storage,
    app_data_dir: &Path,
    provider_id: &str,
) -> Result<(), String> {
    remove_account_with_store(
        storage,
        app_data_dir,
        provider_id,
        delete_account_credential,
    )
}

fn remove_account_with_store<FDelete>(
    storage: &Storage,
    app_data_dir: &Path,
    provider_id: &str,
    delete_secret: FDelete,
) -> Result<(), String>
where
    FDelete: FnOnce(&str) -> Result<(), String>,
{
    if !provider_id.starts_with("antigravity@") || !is_account_provider_id(provider_id) {
        return Err("That account cannot be removed.".to_owned());
    }
    delete_secret(provider_id)?;
    AccessTokenCache::new(account_cache_path(app_data_dir, provider_id)).discard();
    storage
        .delete_provider_account_record(FAMILY, provider_id)
        .map_err(|_| "Account metadata could not be removed.".to_owned())?;
    let _ = storage.delete_snapshot(provider_id);
    Ok(())
}

fn has_refresh_source(
    has_credentials: bool,
    local_server_available: impl FnOnce() -> bool,
) -> bool {
    has_credentials || local_server_available()
}

impl crate::providers::UsageProvider for AntigravityProvider {
    fn definition(&self) -> ProviderDefinition {
        match &self.account {
            AntigravityAccountMode::Default => definition(),
            AntigravityAccountMode::SavedAccount {
                provider_id,
                display_name,
                ..
            } => definition_for(provider_id, display_name),
        }
    }

    fn has_local_credentials(&self) -> bool {
        match &self.account {
            AntigravityAccountMode::Default => {
                has_refresh_source(auth::has_local_credentials(), || discover().is_some())
            }
            AntigravityAccountMode::SavedAccount { provider_id, .. } => {
                has_account_credentials(provider_id)
            }
        }
    }

    fn cache_identity(&self) -> CacheIdentity<'_> {
        match &self.account {
            AntigravityAccountMode::Default => CacheIdentity::Unscoped,
            AntigravityAccountMode::SavedAccount { identity, .. } => {
                CacheIdentity::Resolved(identity.as_str())
            }
        }
    }

    fn account_identity(&self) -> Option<&str> {
        match &self.account {
            AntigravityAccountMode::Default => None,
            AntigravityAccountMode::SavedAccount { identity, .. } => Some(identity.as_str()),
        }
    }

    fn supports_account_names(&self) -> bool {
        true
    }

    fn supports_multi_account_management(&self) -> bool {
        matches!(self.account, AntigravityAccountMode::Default)
    }

    fn is_removable_account(&self) -> bool {
        matches!(self.account, AntigravityAccountMode::SavedAccount { .. })
    }

    fn refresh(&self) -> Result<ProviderSnapshot, crate::providers::ProviderError> {
        self.refresh_inner().map_err(|error| {
            use crate::models::ProviderErrorKind as Kind;

            let kind = match error {
                AntigravityError::NotSignedIn
                | AntigravityError::AuthExpired
                | AntigravityError::InvalidCredentialData => Kind::Authentication,
                AntigravityError::CredentialStoreUnreadable => Kind::CredentialStorage,
                AntigravityError::Unavailable => Kind::Network,
            };
            crate::providers::ProviderError::from_display(kind, error)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use chrono::{Duration, TimeZone, Utc};

    use super::{
        access_token_candidates, auth::AntigravityToken, credential_failure, has_refresh_source,
        should_refresh_access_token, AntigravityError,
    };

    #[test]
    fn keychain_access_token_precedes_a_distinct_cached_token() {
        let now = Utc.with_ymd_and_hms(2026, 7, 18, 12, 0, 0).unwrap();
        let source = AntigravityToken {
            access_token: Some("keychain-access".into()),
            refresh_token: Some("refresh".into()),
            expiry: Some(now + Duration::hours(1)),
        };

        let (candidates, expired) =
            access_token_candidates(&source, Some("cached-access".into()), now);
        assert_eq!(candidates, ["keychain-access", "cached-access"]);
        assert!(!expired);

        let (deduplicated, _) =
            access_token_candidates(&source, Some("keychain-access".into()), now);
        assert_eq!(deduplicated, ["keychain-access"]);
    }

    #[test]
    fn expiring_keychain_tokens_are_skipped_but_count_as_auth_evidence() {
        let now = Utc.with_ymd_and_hms(2026, 7, 18, 12, 0, 0).unwrap();
        let source = AntigravityToken {
            access_token: Some("expiring".into()),
            refresh_token: Some("refresh".into()),
            expiry: Some(now + Duration::seconds(30)),
        };

        let (candidates, expired) = access_token_candidates(&source, None, now);
        assert!(candidates.is_empty());
        assert!(expired);
    }

    #[test]
    fn refresh_and_terminal_error_decisions_preserve_auth_network_distinction() {
        assert!(!should_refresh_access_token(false, true, true));
        assert!(should_refresh_access_token(true, true, true));
        assert!(should_refresh_access_token(false, false, true));
        assert!(!should_refresh_access_token(true, true, false));

        assert!(matches!(
            credential_failure(true, false, true),
            AntigravityError::AuthExpired
        ));
        assert!(matches!(
            credential_failure(false, true, true),
            AntigravityError::Unavailable
        ));
        assert!(matches!(
            credential_failure(false, false, false),
            AntigravityError::NotSignedIn
        ));
    }

    #[test]
    fn running_language_server_is_a_refresh_source_without_credentials() {
        assert!(has_refresh_source(false, || true));
        assert!(!has_refresh_source(false, || false));
    }

    #[test]
    fn credentials_skip_language_server_discovery() {
        let discovery_called = Cell::new(false);

        assert!(has_refresh_source(true, || {
            discovery_called.set(true);
            false
        }));
        assert!(!discovery_called.get());
    }

    #[test]
    fn multi_account_lifecycle_creates_loads_rejects_duplicates_and_removes() {
        use std::{
            collections::{HashMap, HashSet},
            sync::{Arc, Mutex},
        };
        use tempfile::tempdir;

        use crate::storage::Storage;

        let dir = tempdir().unwrap();
        let storage = Storage::open(&dir.path().join("openquota.db")).unwrap();
        let secrets = Arc::new(Mutex::new(HashMap::<String, Vec<u8>>::new()));

        let secrets_save = secrets.clone();
        let secrets_delete = secrets.clone();
        let added = super::create_account_with_store(
            &storage,
            dir.path(),
            &HashSet::new(),
            Some("Work Account"),
            Some("1//0gWorkRefreshToken1234567890"),
            || Ok(None),
            move |id, bytes| {
                secrets_save
                    .lock()
                    .unwrap()
                    .insert(id.to_owned(), bytes.to_vec());
                Ok(())
            },
            move |id| {
                secrets_delete.lock().unwrap().remove(id);
                Ok(())
            },
        )
        .unwrap();

        assert!(added.provider_id.starts_with("antigravity@"));
        assert_eq!(added.custom_name.as_deref(), Some("Work Account"));
        assert!(added.runtime.is_removable_account());
        assert!(!added.runtime.supports_multi_account_management());
        assert!(added.runtime.supports_account_names());
        assert!(added.runtime.account_identity().is_some());
        assert!(secrets.lock().unwrap().contains_key(&added.provider_id));

        // Duplicate credential is rejected cleanly.
        let dup = super::create_account_with_store(
            &storage,
            dir.path(),
            &HashSet::from([added.provider_id.clone()]),
            Some("Duplicate"),
            Some("1//0gWorkRefreshToken1234567890"),
            || Ok(None),
            |_, _| Ok(()),
            |_| Ok(()),
        );
        assert!(dup.is_err());

        // Startup loader restores the saved account runtime.
        let loaded = super::saved_account_runtimes(&storage, dir.path()).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].definition().id, added.provider_id);

        // Removing deletes secret + metadata cleanly.
        let secrets_rm = secrets.clone();
        super::remove_account_with_store(&storage, dir.path(), &added.provider_id, move |id| {
            secrets_rm.lock().unwrap().remove(id);
            Ok(())
        })
        .unwrap();
        assert!(!secrets.lock().unwrap().contains_key(&added.provider_id));
        assert!(super::saved_account_runtimes(&storage, dir.path())
            .unwrap()
            .is_empty());
    }
}
