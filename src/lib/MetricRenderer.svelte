<script lang="ts">
  import { t, translateBackendText, translateMetricLabel } from './i18n.svelte';
  import { usageSourceNote, type ProviderCatalogIndex } from './metrics';
  import QuotaMetric from './QuotaMetric.svelte';
  import StatusMetric from './StatusMetric.svelte';
  import UsageMetric from './UsageMetric.svelte';
  import UsageTrend from './UsageTrend.svelte';
  import ValueMetric from './ValueMetric.svelte';
  import type { AppSettings, MetricLayout, ProviderSnapshot } from './types';

  interface Props {
    layout: MetricLayout;
    snapshot: ProviderSnapshot;
    settings: AppSettings;
    now: number;
    catalog: ProviderCatalogIndex;
    onSettingsChange: (settings: AppSettings) => void;
  }
  let { layout, snapshot, settings, now, catalog, onSettingsChange }: Props = $props();
  const definition = $derived(catalog.metric(layout.id));
  const quota = $derived.by(() => {
    const source = definition?.source;
    if (source?.kind !== 'quota' && source?.kind !== 'quotaOrValue') return undefined;
    return snapshot.quotas.find((item) => item.id === source.sourceId);
  });
  const isSessionWindow = $derived(
    (definition?.source.kind === 'quota' || definition?.source.kind === 'quotaOrValue') &&
      definition.source.sessionWindow,
  );
  const period = $derived.by(() => {
    if (definition?.source.kind !== 'usage') return null;
    if (definition.source.period === 'today') return snapshot.usage.today;
    if (definition.source.period === 'yesterday') return snapshot.usage.yesterday;
    return snapshot.usage.last30Days;
  });
  const valueMetric = $derived.by(() => {
    const source = definition?.source;
    if (source?.kind !== 'value' && source?.kind !== 'quotaOrValue') return null;
    return snapshot.valueMetrics.find((item) => item.id === source.sourceId) ?? null;
  });
  const statusMetric = $derived.by(() => {
    const source = definition?.source;
    if (source?.kind !== 'status') return null;
    return snapshot.statusMetrics.find((item) => item.id === source.sourceId) ?? null;
  });
  const resolvedUsageSourceNote = $derived(
    translateBackendText(usageSourceNote(catalog, snapshot)),
  );
  const displayLabel = $derived(definition ? translateMetricLabel(definition.label) : '');
</script>

{#if (definition?.source.kind === 'quota' || definition?.source.kind === 'quotaOrValue') && quota}
  <QuotaMetric
    {quota}
    {now}
    usageDisplay={settings.usageDisplay}
    resetDisplay={settings.resetDisplay}
    timeFormat={settings.timeFormat}
    alwaysShowPacing={settings.alwaysShowPacing}
    {isSessionWindow}
    onToggleUsage={() =>
      onSettingsChange({
        ...settings,
        usageDisplay: settings.usageDisplay === 'used' ? 'left' : 'used',
      })}
    onToggleReset={() =>
      onSettingsChange({
        ...settings,
        resetDisplay: settings.resetDisplay === 'countdown' ? 'exact' : 'countdown',
      })}
  />
{:else if definition?.source.kind === 'quotaOrValue' && valueMetric}
  <ValueMetric
    label={displayLabel}
    metric={valueMetric}
    {now}
    resetDisplay={settings.resetDisplay}
    timeFormat={settings.timeFormat}
  />
{:else if definition?.source.kind === 'quota' || definition?.source.kind === 'quotaOrValue'}
  <section class="metric metric--no-data" aria-label={t('quota.aria', { label: displayLabel })}>
    <div class="metric__heading"><h2>{displayLabel}</h2></div>
    <div class="meter-shell">
      <div
        class="meter"
        role="progressbar"
        aria-label={t('quota.usedAria', { label: displayLabel })}
        aria-valuemin="0"
        aria-valuemax="100"
        aria-valuenow="0"
      ></div>
    </div>
    <div class="metric__reading">
      <span>{t('common.noData')}</span><span>{t('quota.resetUnavailable')}</span>
    </div>
  </section>
{:else if definition?.source.kind === 'trend'}
  <UsageTrend daily={snapshot.usage.daily} sourceNote={resolvedUsageSourceNote} />
{:else if definition?.source.kind === 'status'}
  <StatusMetric label={displayLabel} metric={statusMetric} />
{:else if definition?.source.kind === 'usage'}
  <UsageMetric label={displayLabel} {period} />
{:else if definition?.source.kind === 'value'}
  <ValueMetric
    label={displayLabel}
    metric={valueMetric}
    {now}
    resetDisplay={settings.resetDisplay}
    timeFormat={settings.timeFormat}
  />
{/if}
