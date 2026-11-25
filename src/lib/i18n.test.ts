import { cleanup, render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  dictionaries,
  i18n,
  t,
  translateBackendText,
  translateLinkLabel,
  translateMetricLabel,
} from './i18n.svelte';
import { formatReset } from './pacing';
import QuotaMetric from './QuotaMetric.svelte';
import SettingsScreen from './SettingsScreen.svelte';
import { settingsState } from '../test/appFixtures';

afterEach(() => {
  cleanup();
  i18n.setPreference('system');
});

describe('i18n localization', () => {
  it('provides a Chinese translation for every English message key', () => {
    const enKeys = Object.keys(dictionaries.en).sort();
    const zhKeys = Object.keys(dictionaries['zh-CN']).sort();
    expect(zhKeys).toEqual(enKeys);
    for (const key of enKeys) {
      expect(
        dictionaries['zh-CN'][key as keyof typeof dictionaries.en].trim().length,
      ).toBeGreaterThan(0);
    }
  });

  it('switches reactively between Simplified Chinese and English', () => {
    i18n.setPreference('zhCn');
    expect(i18n.locale).toBe('zh-CN');
    expect(t('settings.appearance')).toBe('外观');
    expect(translateMetricLabel('Session')).toBe('会话');
    expect(translateLinkLabel('Dashboard')).toBe('控制台');
    expect(translateBackendText('Start Antigravity or run `agy` and try again.')).toBe(
      '请先启动 Antigravity 或运行 `agy` 后重试。',
    );
    expect(
      formatReset(
        new Date(Date.UTC(2026, 6, 12, 12, 30, 0)).toISOString(),
        Date.UTC(2026, 6, 12, 10, 0, 0),
        'countdown',
      ),
    ).toBe('2小时 30分后重置');

    i18n.setPreference('en');
    expect(i18n.locale).toBe('en');
    expect(t('settings.appearance')).toBe('Appearance');
    expect(translateMetricLabel('Session')).toBe('Session');
  });

  it('renders SettingsScreen and QuotaMetric in Simplified Chinese when selected', () => {
    i18n.setPreference('zhCn');
    render(SettingsScreen, {
      settingsView: {
        ...settingsState,
        settings: { ...settingsState.settings, language: 'zhCn' },
      },
      platform: 'windows',
      panelHeightMode: 'automatic',
      onChange: vi.fn(),
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
    expect(screen.getByText('外观')).toBeInTheDocument();
    expect(screen.getByText('语言')).toBeInTheDocument();
    expect(screen.getByText('额度显示')).toBeInTheDocument();
    cleanup();

    render(QuotaMetric, {
      quota: {
        id: 'geminiPro',
        label: 'Session',
        usedPercent: 25,
        resetsAt: new Date(Date.UTC(2026, 6, 12, 13, 0, 0)).toISOString(),
        periodSeconds: 18000,
        format: 'percent',
        usedValue: null,
        limitValue: null,
        estimated: false,
      },
      now: Date.UTC(2026, 6, 12, 10, 0, 0),
      usageDisplay: 'left',
      resetDisplay: 'countdown',
      timeFormat: 'twentyFourHour',
      alwaysShowPacing: false,
      onToggleUsage: vi.fn(),
      onToggleReset: vi.fn(),
    });
    expect(screen.getByText('会话')).toBeInTheDocument();
    expect(screen.getByText('剩余 75%')).toBeInTheDocument();
    expect(screen.getByText('3小时后重置')).toBeInTheDocument();
  });
});
