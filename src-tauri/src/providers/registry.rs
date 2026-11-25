use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, RwLock},
};

use crate::models::{MetricSection, MetricSource, ProviderCatalog, ProviderDefinition};

use super::{is_account_provider_id, provider_family, OwnedCacheIdentity, UsageProvider};

const MAX_DEFAULT_PINS: usize = 2;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProviderRegistryError {
    #[error("Provider registry contains no providers.")]
    Empty,
    #[error("Provider definition is invalid: {0}")]
    Invalid(String),
}

struct RegistryState {
    runtimes: HashMap<String, Arc<dyn UsageProvider>>,
    catalog: ProviderCatalog,
    definition_indices: HashMap<String, usize>,
    metric_indices: HashMap<String, (usize, usize)>,
}

pub struct ProviderRegistry {
    state: RwLock<RegistryState>,
}

impl ProviderRegistry {
    pub fn new(providers: Vec<Arc<dyn UsageProvider>>) -> Result<Self, ProviderRegistryError> {
        if providers.is_empty() {
            return Err(ProviderRegistryError::Empty);
        }

        let mut runtimes = HashMap::new();
        let mut definitions = Vec::with_capacity(providers.len());
        let mut metric_owners = BTreeMap::<String, String>::new();

        for provider in providers {
            let definition = normalized_definition(provider.as_ref());
            if runtimes.contains_key(&definition.id) {
                return Err(invalid(format!(
                    "duplicate provider id `{}`",
                    definition.id
                )));
            }
            validate_definition(&definition, &metric_owners)?;
            for metric in &definition.metrics {
                metric_owners.insert(metric.id.clone(), definition.id.clone());
            }
            runtimes.insert(definition.id.clone(), provider);
            definitions.push(definition);
        }
        if !definitions
            .iter()
            .any(|definition| definition.fallback_enabled)
        {
            return Err(invalid("registry has no fallback-enabled provider"));
        }

        let mut state = RegistryState {
            runtimes,
            catalog: ProviderCatalog {
                providers: definitions,
                api_key_provider_ids: Vec::new(),
                multi_account_provider_ids: Vec::new(),
                removable_provider_ids: Vec::new(),
            },
            definition_indices: HashMap::new(),
            metric_indices: HashMap::new(),
        };
        rebuild_indices(&mut state);

        Ok(Self {
            state: RwLock::new(state),
        })
    }

    pub fn register(
        &self,
        provider: Arc<dyn UsageProvider>,
    ) -> Result<ProviderCatalog, ProviderRegistryError> {
        let definition = normalized_definition(provider.as_ref());
        let mut state = self
            .state
            .write()
            .map_err(|_| invalid("provider registry lock is unavailable"))?;
        if state.runtimes.contains_key(&definition.id) {
            return Err(invalid(format!(
                "duplicate provider id `{}`",
                definition.id
            )));
        }
        let metric_owners = state
            .catalog
            .providers
            .iter()
            .flat_map(|existing| {
                existing
                    .metrics
                    .iter()
                    .map(|metric| (metric.id.clone(), existing.id.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        validate_definition(&definition, &metric_owners)?;

        if is_account_provider_id(&definition.id) {
            let family = provider_family(&definition.id);
            let insert_at = state
                .catalog
                .providers
                .iter()
                .rposition(|existing| provider_family(&existing.id) == family)
                .map_or(state.catalog.providers.len(), |index| index + 1);
            state
                .catalog
                .providers
                .insert(insert_at, definition.clone());
        } else {
            state.catalog.providers.push(definition.clone());
        }
        state.runtimes.insert(definition.id, provider);
        rebuild_indices(&mut state);
        Ok(state.catalog.clone())
    }

    pub fn unregister(&self, provider_id: &str) -> Result<ProviderCatalog, ProviderRegistryError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| invalid("provider registry lock is unavailable"))?;
        let Some(runtime) = state.runtimes.get(provider_id) else {
            return Err(invalid(format!("unknown provider id `{provider_id}`")));
        };
        if !runtime.is_removable_account() {
            return Err(invalid(format!(
                "provider `{provider_id}` cannot be removed"
            )));
        }
        let has_remaining_fallback = state
            .catalog
            .providers
            .iter()
            .any(|definition| definition.id != provider_id && definition.fallback_enabled);
        if !has_remaining_fallback {
            return Err(invalid("registry has no fallback-enabled provider"));
        }

        state.runtimes.remove(provider_id);
        state
            .catalog
            .providers
            .retain(|definition| definition.id != provider_id);
        rebuild_indices(&mut state);
        Ok(state.catalog.clone())
    }

    pub fn catalog(&self) -> ProviderCatalog {
        self.state
            .read()
            .map(|state| state.catalog.clone())
            .unwrap_or_default()
    }

    pub fn runtime(&self, id: &str) -> Option<Arc<dyn UsageProvider>> {
        self.state
            .read()
            .ok()
            .and_then(|state| state.runtimes.get(id).cloned())
    }

    pub fn definition(&self, id: &str) -> Option<ProviderDefinition> {
        let state = self.state.read().ok()?;
        let index = *state.definition_indices.get(id)?;
        state.catalog.providers.get(index).cloned()
    }

    pub fn cache_identity(&self, id: &str) -> OwnedCacheIdentity {
        let Some(runtime) = self.runtime(id) else {
            return OwnedCacheIdentity::Unscoped;
        };
        OwnedCacheIdentity::from_borrowed(runtime.cache_identity())
    }

    pub fn supports_account_names(&self, id: &str) -> bool {
        self.runtime(id)
            .is_some_and(|runtime| runtime.supports_account_names())
    }

    pub fn supports_multi_account_management(&self, id: &str) -> bool {
        self.runtime(id)
            .is_some_and(|runtime| runtime.supports_multi_account_management())
    }

    pub fn is_removable_account(&self, id: &str) -> bool {
        self.runtime(id)
            .is_some_and(|runtime| runtime.is_removable_account())
    }

    pub fn observed_account_provider_ids(&self) -> Vec<String> {
        let Ok(state) = self.state.read() else {
            return Vec::new();
        };
        state
            .catalog
            .providers
            .iter()
            .filter(|definition| {
                state
                    .runtimes
                    .get(&definition.id)
                    .is_some_and(|runtime| runtime.account_identity().is_some())
            })
            .map(|definition| definition.id.clone())
            .collect()
    }

    pub fn metric(&self, id: &str) -> Option<crate::models::MetricDefinition> {
        let state = self.state.read().ok()?;
        let (provider_index, metric_index) = *state.metric_indices.get(id)?;
        state
            .catalog
            .providers
            .get(provider_index)?
            .metrics
            .get(metric_index)
            .cloned()
    }

    #[cfg(test)]
    pub fn from_definitions(
        definitions: Vec<ProviderDefinition>,
    ) -> Result<Self, ProviderRegistryError> {
        Self::new(
            definitions
                .into_iter()
                .map(|definition| {
                    Arc::new(DefinitionOnlyProvider(definition)) as Arc<dyn UsageProvider>
                })
                .collect(),
        )
    }
}

fn normalized_definition(provider: &dyn UsageProvider) -> ProviderDefinition {
    let mut definition = provider.definition();
    definition.links = definition
        .links
        .iter()
        .filter_map(crate::models::ProviderLink::visible)
        .collect();
    definition
}

fn rebuild_indices(state: &mut RegistryState) {
    state.definition_indices.clear();
    state.metric_indices.clear();
    state.catalog.api_key_provider_ids.clear();
    state.catalog.multi_account_provider_ids.clear();
    state.catalog.removable_provider_ids.clear();

    for (provider_index, definition) in state.catalog.providers.iter().enumerate() {
        state
            .definition_indices
            .insert(definition.id.clone(), provider_index);
        for (metric_index, metric) in definition.metrics.iter().enumerate() {
            state
                .metric_indices
                .insert(metric.id.clone(), (provider_index, metric_index));
        }
        if let Some(runtime) = state.runtimes.get(&definition.id) {
            if runtime.supports_api_key_configuration() {
                state
                    .catalog
                    .api_key_provider_ids
                    .push(definition.id.clone());
            }
            if runtime.supports_multi_account_management() {
                state
                    .catalog
                    .multi_account_provider_ids
                    .push(definition.id.clone());
            }
            if runtime.is_removable_account() {
                state
                    .catalog
                    .removable_provider_ids
                    .push(definition.id.clone());
            }
        }
    }
}

#[cfg(test)]
struct DefinitionOnlyProvider(ProviderDefinition);

#[cfg(test)]
impl UsageProvider for DefinitionOnlyProvider {
    fn definition(&self) -> ProviderDefinition {
        self.0.clone()
    }

    fn supports_account_names(&self) -> bool {
        matches!(super::provider_family(&self.0.id), "claude" | "codex")
    }

    fn has_local_credentials(&self) -> bool {
        false
    }

    fn refresh(&self) -> Result<crate::models::ProviderSnapshot, super::ProviderError> {
        unreachable!()
    }
}

fn validate_definition(
    provider: &ProviderDefinition,
    metric_owners: &BTreeMap<String, String>,
) -> Result<(), ProviderRegistryError> {
    if provider.id.trim().is_empty() {
        return Err(invalid("provider id is empty"));
    }
    if provider.display_name.trim().is_empty() {
        return Err(invalid(format!(
            "provider `{}` has no display name",
            provider.id
        )));
    }
    if provider.short_name.trim().is_empty() {
        return Err(invalid(format!(
            "provider `{}` has no tray short name",
            provider.id
        )));
    }
    if provider.metrics.is_empty() {
        return Err(invalid(format!(
            "provider `{}` has no metrics",
            provider.id
        )));
    }

    let prefix = format!("{}.", provider.id);
    let mut local_ids = BTreeMap::<&str, ()>::new();
    let mut default_pins = 0;
    let mut has_visible_metric = false;

    for metric in &provider.metrics {
        if !metric.id.starts_with(&prefix) || metric.id.len() == prefix.len() {
            return Err(invalid(format!(
                "metric `{}` must use provider prefix `{prefix}`",
                metric.id
            )));
        }
        if local_ids.insert(metric.id.as_str(), ()).is_some()
            || metric_owners.contains_key(&metric.id)
        {
            return Err(invalid(format!("duplicate metric id `{}`", metric.id)));
        }
        if metric.label.trim().is_empty() {
            return Err(invalid(format!("metric `{}` has no label", metric.id)));
        }
        if metric
            .source
            .source_id()
            .is_some_and(|source| source.trim().is_empty())
        {
            return Err(invalid(format!(
                "metric `{}` has an empty source id",
                metric.id
            )));
        }
        if matches!(metric.source, MetricSource::Trend) && metric.pinnable {
            return Err(invalid(format!(
                "trend metric `{}` cannot be pinnable",
                metric.id
            )));
        }
        if metric.default_pinned && (!metric.pinnable || !metric.default_enabled) {
            return Err(invalid(format!(
                "metric `{}` has an invalid default pin",
                metric.id
            )));
        }
        if metric.pinnable != metric.tray.is_some() {
            return Err(invalid(format!(
                "metric `{}` has inconsistent tray metadata",
                metric.id
            )));
        }
        if metric
            .tray
            .as_ref()
            .is_some_and(|tray| tray.short_label.trim().is_empty())
        {
            return Err(invalid(format!(
                "metric `{}` has an empty tray label",
                metric.id
            )));
        }
        default_pins += usize::from(metric.default_pinned);
        has_visible_metric |=
            metric.default_enabled && metric.default_section == MetricSection::AlwaysVisible;
    }

    if default_pins > MAX_DEFAULT_PINS {
        return Err(invalid(format!(
            "provider `{}` has more than {MAX_DEFAULT_PINS} default pins",
            provider.id
        )));
    }
    if !has_visible_metric {
        return Err(invalid(format!(
            "provider `{}` has no default always-visible metric",
            provider.id
        )));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> ProviderRegistryError {
    ProviderRegistryError::Invalid(message.into())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{
        models::{
            MetricDefinition, MetricSection, MetricSource, ProviderDefinition, ProviderSnapshot,
        },
        providers::{ProviderError, UsageProvider},
    };

    use super::{ProviderRegistry, ProviderRegistryError};

    struct StubProvider(ProviderDefinition);

    struct ApiKeyStubProvider(ProviderDefinition);

    impl UsageProvider for StubProvider {
        fn definition(&self) -> ProviderDefinition {
            self.0.clone()
        }

        fn has_local_credentials(&self) -> bool {
            false
        }

        fn refresh(&self) -> Result<ProviderSnapshot, ProviderError> {
            unreachable!()
        }
    }

    impl UsageProvider for ApiKeyStubProvider {
        fn definition(&self) -> ProviderDefinition {
            self.0.clone()
        }

        fn has_local_credentials(&self) -> bool {
            false
        }

        fn supports_api_key_configuration(&self) -> bool {
            true
        }

        fn refresh(&self) -> Result<ProviderSnapshot, ProviderError> {
            unreachable!()
        }
    }

    fn definition(id: &str) -> ProviderDefinition {
        ProviderDefinition {
            id: id.into(),
            display_name: "Provider".into(),
            short_name: "P".into(),
            fallback_enabled: true,
            dynamic_quotas: false,
            local_usage_source_note: None,
            links: vec![],
            metrics: vec![MetricDefinition::new(
                format!("{id}.session"),
                "Session",
                MetricSource::Quota {
                    source_id: "session".into(),
                    session_window: true,
                },
                true,
                true,
                MetricSection::AlwaysVisible,
                true,
                Some("S"),
                None,
            )],
        }
    }

    fn runtime(definition: ProviderDefinition) -> Arc<dyn UsageProvider> {
        Arc::new(StubProvider(definition))
    }

    #[test]
    fn registry_preserves_definition_order_and_indexes_runtimes() {
        let registry = ProviderRegistry::new(vec![
            runtime(definition("first")),
            runtime(definition("second")),
        ])
        .unwrap();

        assert_eq!(
            registry
                .catalog()
                .providers
                .iter()
                .map(|provider| provider.id.as_str())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert!(registry.runtime("second").is_some());
        assert!(registry.definition("first").is_some());
        assert!(registry.metric("first.session").is_some());
    }

    #[test]
    fn registry_exposes_api_key_configuration_capabilities() {
        let registry = ProviderRegistry::new(vec![
            runtime(definition("local")),
            Arc::new(ApiKeyStubProvider(definition("keyed"))),
        ])
        .unwrap();

        assert_eq!(registry.catalog().api_key_provider_ids, ["keyed"]);
    }

    #[test]
    fn registry_exposes_only_trimmed_http_provider_links() {
        let mut provider = definition("links");
        provider.links = vec![
            crate::models::ProviderLink::new(" Status ", " https://status.example.com/ "),
            crate::models::ProviderLink::new("", "https://example.com/"),
            crate::models::ProviderLink::new("File", "file:///tmp/private"),
        ];

        let registry = ProviderRegistry::new(vec![runtime(provider)]).unwrap();

        assert_eq!(
            registry.definition("links").unwrap().links,
            vec![crate::models::ProviderLink::new(
                "Status",
                "https://status.example.com/"
            )]
        );
    }

    #[test]
    fn registry_rejects_duplicate_provider_and_metric_ids() {
        let duplicate_provider = ProviderRegistry::new(vec![
            runtime(definition("same")),
            runtime(definition("same")),
        ]);
        assert!(matches!(
            duplicate_provider,
            Err(ProviderRegistryError::Invalid(message)) if message.contains("duplicate provider")
        ));

        let mut duplicated = definition("metrics");
        duplicated.metrics.push(duplicated.metrics[0].clone());
        let duplicate_metric = ProviderRegistry::new(vec![runtime(duplicated)]);
        assert!(matches!(
            duplicate_metric,
            Err(ProviderRegistryError::Invalid(message)) if message.contains("duplicate metric")
        ));
    }

    #[test]
    fn registry_rejects_invalid_defaults_and_sources() {
        let mut invalid_pin = definition("pin");
        invalid_pin.metrics[0].pinnable = false;
        assert!(matches!(
            ProviderRegistry::new(vec![runtime(invalid_pin)]),
            Err(ProviderRegistryError::Invalid(message)) if message.contains("default pin")
        ));

        let mut hidden = definition("hidden");
        hidden.metrics[0].default_section = MetricSection::OnDemand;
        hidden.metrics[0].default_pinned = false;
        assert!(matches!(
            ProviderRegistry::new(vec![runtime(hidden)]),
            Err(ProviderRegistryError::Invalid(message)) if message.contains("always-visible")
        ));

        let mut empty_source = definition("source");
        empty_source.metrics[0].source = MetricSource::Quota {
            source_id: " ".into(),
            session_window: false,
        };
        assert!(matches!(
            ProviderRegistry::new(vec![runtime(empty_source)]),
            Err(ProviderRegistryError::Invalid(message)) if message.contains("empty source")
        ));

        let mut wrong_prefix = definition("prefix");
        wrong_prefix.metrics[0].id = "other.session".into();
        assert!(matches!(
            ProviderRegistry::new(vec![runtime(wrong_prefix)]),
            Err(ProviderRegistryError::Invalid(message)) if message.contains("provider prefix")
        ));

        let mut trend = definition("trend");
        trend.metrics[0].source = MetricSource::Trend;
        assert!(matches!(
            ProviderRegistry::new(vec![runtime(trend)]),
            Err(ProviderRegistryError::Invalid(message)) if message.contains("cannot be pinnable")
        ));

        let mut empty_tray = definition("tray");
        empty_tray.metrics[0].tray.as_mut().unwrap().short_label = " ".into();
        assert!(matches!(
            ProviderRegistry::new(vec![runtime(empty_tray)]),
            Err(ProviderRegistryError::Invalid(message)) if message.contains("empty tray label")
        ));

        let mut missing_tray = definition("missing-tray");
        missing_tray.metrics[0].tray = None;
        assert!(matches!(
            ProviderRegistry::new(vec![runtime(missing_tray)]),
            Err(ProviderRegistryError::Invalid(message)) if message.contains("inconsistent tray metadata")
        ));

        let mut no_fallback = definition("no-fallback");
        no_fallback.fallback_enabled = false;
        assert!(matches!(
            ProviderRegistry::new(vec![runtime(no_fallback)]),
            Err(ProviderRegistryError::Invalid(message)) if message.contains("no fallback-enabled")
        ));

        let mut too_many_pins = definition("pins");
        for suffix in ["weekly", "monthly"] {
            let mut metric = too_many_pins.metrics[0].clone();
            metric.id = format!("pins.{suffix}");
            if let MetricSource::Quota { source_id, .. } = &mut metric.source {
                *source_id = suffix.to_owned();
            }
            too_many_pins.metrics.push(metric);
        }
        assert!(matches!(
            ProviderRegistry::new(vec![runtime(too_many_pins)]),
            Err(ProviderRegistryError::Invalid(message)) if message.contains("more than 2 default pins")
        ));
    }

    #[test]
    fn builtin_provider_catalog_keeps_the_product_defaults() {
        use crate::providers::{
            antigravity, claude, codex, copilot, cursor, devin, grok, opencode, openrouter, zai,
        };

        let registry = ProviderRegistry::new(vec![
            runtime(claude::definition()),
            runtime(codex::definition()),
            runtime(cursor::definition()),
            runtime(antigravity::definition()),
            runtime(copilot::definition()),
            runtime(devin::definition()),
            runtime(grok::definition()),
            runtime(opencode::definition()),
            runtime(openrouter::definition()),
            runtime(zai::definition()),
        ])
        .unwrap();
        let catalog = registry.catalog();

        assert_eq!(
            catalog
                .providers
                .iter()
                .map(|provider| provider.id.as_str())
                .collect::<Vec<_>>(),
            [
                "claude",
                "codex",
                "cursor",
                "antigravity",
                "copilot",
                "devin",
                "grok",
                "opencode",
                "openrouter",
                "zai",
            ]
        );
        assert_eq!(
            registry
                .definition("codex")
                .unwrap()
                .metrics
                .iter()
                .map(|metric| metric.id.as_str())
                .collect::<Vec<_>>(),
            [
                "codex.session",
                "codex.weekly",
                "codex.spark",
                "codex.sparkWeekly",
                "codex.trend",
                "codex.credits",
                "codex.rateLimitResets",
                "codex.today",
                "codex.yesterday",
                "codex.last30",
            ]
        );
        assert!(registry.definition("codex").unwrap().fallback_enabled);
        assert!(registry.definition("claude").unwrap().fallback_enabled);
        assert!(registry.definition("cursor").unwrap().fallback_enabled);
        for provider_id in [
            "antigravity",
            "copilot",
            "devin",
            "grok",
            "opencode",
            "openrouter",
            "zai",
        ] {
            assert!(!registry.definition(provider_id).unwrap().fallback_enabled);
        }
        assert!(registry
            .metric("claude.session")
            .unwrap()
            .source
            .session_window());

        let serialized = serde_json::to_value(catalog).unwrap();
        assert_eq!(serialized["providers"][1]["displayName"], "Codex");
        assert_eq!(
            serialized["providers"][1]["metrics"][6]["source"],
            serde_json::json!({"kind":"value","sourceId":"rateLimitResets"})
        );
        assert_eq!(
            serialized["providers"][1]["metrics"][9]["source"],
            serde_json::json!({"kind":"usage","period":"last30Days"})
        );
    }

    struct RemovableAccountStub {
        definition: ProviderDefinition,
        multi_account: bool,
        removable: bool,
    }

    impl UsageProvider for RemovableAccountStub {
        fn definition(&self) -> ProviderDefinition {
            self.definition.clone()
        }

        fn has_local_credentials(&self) -> bool {
            true
        }

        fn supports_account_names(&self) -> bool {
            true
        }

        fn supports_multi_account_management(&self) -> bool {
            self.multi_account
        }

        fn is_removable_account(&self) -> bool {
            self.removable
        }

        fn refresh(&self) -> Result<ProviderSnapshot, ProviderError> {
            unreachable!()
        }
    }

    #[test]
    fn registry_hot_registers_and_unregisters_removable_account_providers() {
        let mut base_ag = definition("antigravity");
        base_ag.fallback_enabled = false;
        let base_other = definition("codex");
        let registry = ProviderRegistry::new(vec![
            Arc::new(RemovableAccountStub {
                definition: base_ag,
                multi_account: true,
                removable: false,
            }),
            runtime(base_other),
        ])
        .unwrap();

        assert_eq!(
            registry.catalog().multi_account_provider_ids,
            ["antigravity"]
        );
        assert!(registry.catalog().removable_provider_ids.is_empty());

        let mut account_def = definition("antigravity@1234abcd");
        account_def.fallback_enabled = false;
        let catalog = registry
            .register(Arc::new(RemovableAccountStub {
                definition: account_def,
                multi_account: false,
                removable: true,
            }))
            .unwrap();
        assert_eq!(
            catalog
                .providers
                .iter()
                .map(|provider| provider.id.as_str())
                .collect::<Vec<_>>(),
            ["antigravity", "antigravity@1234abcd", "codex"]
        );
        assert_eq!(catalog.removable_provider_ids, ["antigravity@1234abcd"]);
        assert!(registry.metric("antigravity@1234abcd.session").is_some());

        // Rejects removing base non-removable provider.
        assert!(registry.unregister("antigravity").is_err());

        // Removes added account provider cleanly and re-indexes remaining providers.
        let after_remove = registry.unregister("antigravity@1234abcd").unwrap();
        assert_eq!(
            after_remove
                .providers
                .iter()
                .map(|provider| provider.id.as_str())
                .collect::<Vec<_>>(),
            ["antigravity", "codex"]
        );
        assert!(after_remove.removable_provider_ids.is_empty());
        assert!(registry.definition("antigravity@1234abcd").is_none());
        assert!(registry.metric("antigravity@1234abcd.session").is_none());
        assert!(registry.definition("codex").is_some());
    }
}
