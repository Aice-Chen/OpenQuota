import { cleanup, render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  dictionaries,
  i18n,
  sharedDictionary,
  t,
  translateBackendText,
  translateLinkLabel,
  translateMetricLabel,
  translateUnit,
  translateUpdateFailure,
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

  it('translates all 12 provider backend errors, shared metrics, and updater error codes in Chinese', () => {
    i18n.setPreference('zhCn');
    expect(sharedDictionary.metricLabels['Web Searches']).toBe('联网搜索');
    expect(translateMetricLabel('Total Usage')).toBe('总用量');
    expect(translateMetricLabel('Org Spend')).toBe('组织支出');
    expect(translateUnit('searches')).toBe('次搜索');

    expect(translateBackendText('Not logged in. Run `codex` to authenticate.')).toBe(
      '尚未登录，请运行 `codex` 完成认证。',
    );
    expect(translateBackendText('Could not reach GitHub. Check your internet connection.')).toBe(
      '无法连接到 GitHub，请检查网络连接。',
    );
    expect(translateBackendText('Cursor usage request failed (HTTP 429).')).toBe(
      'Cursor 请求失败（HTTP 429）。',
    );
    expect(translateBackendText('Add a Kimi API key in Customize or set KIMI_API_KEY.')).toBe(
      '请在「自定义」中添加 Kimi API 密钥，或设置环境变量 KIMI_API_KEY。',
    );

    const failure = translateUpdateFailure({
      code: 'signature_invalid',
      message: 'The downloaded update failed its security check.',
      action: 'Do not install this download.',
      retryable: false,
    });
    expect(failure.message).toBe('下载的更新包未通过安全签名校验。');
    expect(failure.action).toContain('请勿安装该文件');
    expect(
      translateUpdateFailure({
        code: 'invalid_endpoint',
        message: 'The custom update source must be a valid HTTPS URL.',
        action: 'Check settings.',
        retryable: false,
      }).message,
    ).toBe('自定义更新源必须是有效的 HTTPS 地址。');

    i18n.setPreference('en');
    expect(
      translateUpdateFailure({
        code: 'signature_invalid',
        message: 'Raw English message.',
        action: 'Raw English action.',
        retryable: false,
      }).message,
    ).toBe('Raw English message.');
  });
});
