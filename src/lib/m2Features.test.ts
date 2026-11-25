import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { antigravityState, providerCatalogIndex, settingsState } from '../test/appFixtures';
import Dashboard from './Dashboard.svelte';
import { i18n } from './i18n.svelte';
import SettingsScreen from './SettingsScreen.svelte';
import { buildProviderShareRows } from './shareCard';
import type { AppSettings, ProviderSnapshot, UsageViewState } from './types';

afterEach(() => {
  cleanup();
  i18n.setPreference('en');
});

describe('M2 enhanced features', () => {
  it('auto-renders dynamic server quota buckets on Dashboard and share cards', () => {
    i18n.setPreference('en');
    const dynamicSnapshot: ProviderSnapshot = {
      ...antigravityState.snapshot!,
      quotas: [
        ...antigravityState.snapshot!.quotas,
        {
          id: 'dyn:veo-daily',
          label: 'Veo Video · Daily Limit',
          usedPercent: 65,
          resetsAt: '2099-01-02T00:00:00Z',
          periodSeconds: 86_400,
          format: 'percent',
          usedValue: null,
          limitValue: null,
          estimated: false,
        },
      ],
    };
    const viewState: UsageViewState = {
      providers: {
        antigravity: {
          ...antigravityState,
          snapshot: dynamicSnapshot,
        },
      },
    };
    const layout = {
      id: 'antigravity',
      enabled: true,
      detected: true,
      expanded: false,
      metrics: [
        {
          id: 'antigravity.geminiPro',
          enabled: true,
          section: 'alwaysVisible' as const,
          pinned: true,
        },
        {
          id: 'antigravity.geminiWeekly',
          enabled: true,
          section: 'alwaysVisible' as const,
          pinned: true,
        },
      ],
    };
    const settings: AppSettings = {
      ...settingsState.settings,
      providers: [layout],
    };

    render(Dashboard, {
      viewState,
      settings,
      now: Date.parse('2026-11-26T00:00:00Z'),
      catalog: providerCatalogIndex,
      renamableProviderIds: [],
      onSettingsChange: vi.fn(),
      onCustomizationChange: vi.fn(),
      onReorderStart: vi.fn(),
      onReorderEnd: vi.fn(),
      onCustomize: vi.fn(),
      onOpenProviderCustomize: vi.fn(),
      onRenameProvider: vi.fn(),
      onShare: vi.fn(),
      onShareTotal: vi.fn().mockResolvedValue(true),
      onRefresh: vi.fn(),
      onOpenProviderLink: vi.fn(),
      onContentMorph: vi.fn(),
      reducedMotion: true,
      updateStatus: null,
      installingUpdate: false,
      updateProgress: null,
      updateError: null,
      onInstallUpdate: vi.fn(),
      onOpenUpdatePage: vi.fn(),
    });

    expect(screen.getByText('Veo Video · Daily Limit')).toBeInTheDocument();
    expect(screen.getByText('35% left')).toBeInTheDocument();

    const shareRows = buildProviderShareRows(
      providerCatalogIndex,
      dynamicSnapshot,
      layout,
      settings,
      Date.parse('2026-11-26T00:00:00Z'),
    );
    expect(
      shareRows.some((row) => row.kind === 'quota' && row.label === 'Veo Video · Daily Limit'),
    ).toBe(true);
  });

  it('validates and saves custom HTTPS update source, refresh interval, and panel width', async () => {
    i18n.setPreference('en');
    const onChange = vi.fn();
    render(SettingsScreen, {
      settingsView: settingsState,
      platform: 'windows',
      panelHeightMode: 'automatic',
      onChange,
      onPanelHeightModeChange: vi.fn(),
      onRequestNotifications: vi.fn(),
      onOpenNotificationSettings: vi.fn(),
      updateError: null,
      checkingUpdate: false,
      onCheckForUpdates: vi.fn(),
      onCustomize: vi.fn(),
      onCopyLogPath: vi.fn().mockResolvedValue(undefined),
      onOpenLogFolder: vi.fn().mockResolvedValue(undefined),
      onResetAllSettings: vi.fn(),
    });

    await fireEvent.click(screen.getByRole('combobox', { name: 'Refresh Interval' }));
    await fireEvent.click(screen.getByRole('option', { name: '10 min' }));
    expect(onChange).toHaveBeenLastCalledWith(
      expect.objectContaining({ refreshIntervalSeconds: 600 }),
    );

    await fireEvent.click(screen.getByRole('combobox', { name: 'Panel Width' }));
    await fireEvent.click(screen.getByRole('option', { name: 'Wide (400px)' }));
    expect(onChange).toHaveBeenLastCalledWith(expect.objectContaining({ panelWidth: 400 }));

    await fireEvent.click(screen.getByRole('combobox', { name: 'Update Source' }));
    await fireEvent.click(screen.getByRole('option', { name: 'Custom Mirror' }));
    const input = screen.getByRole('textbox', { name: 'Custom update manifest URL' });
    await fireEvent.input(input, { target: { value: 'http://insecure.example/latest.json' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(screen.getByText('Enter a valid HTTPS URL.')).toBeInTheDocument();
    expect(onChange).toHaveBeenCalledTimes(2);

    await fireEvent.input(input, { target: { value: 'https://mirror.example.com/latest.json' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(onChange).toHaveBeenLastCalledWith(
      expect.objectContaining({ updateEndpoint: 'https://mirror.example.com/latest.json' }),
    );
  });
});
