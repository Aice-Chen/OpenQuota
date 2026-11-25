import sharedI18n from '../../shared/i18n.json';
import en from './locales/en.json';
import zhCN from './locales/zh-CN.json';
import type { LanguagePreference, UpdateFailure } from './types';

export type Locale = 'en' | 'zh-CN';
export type MessageKey = keyof typeof en;

const METRIC_LABELS_ZH: Record<string, string> = sharedI18n.metricLabels;
const LINK_LABELS_ZH: Record<string, string> = sharedI18n.linkLabels;
const UNITS_ZH: Record<string, string> = sharedI18n.units;

const BACKEND_MESSAGES_ZH: Record<string, string> = {
  // Antigravity
  'Start Antigravity or run `agy` and try again.': '请先启动 Antigravity 或运行 `agy` 后重试。',
  'Antigravity sign-in expired. Open Antigravity or run `agy` to refresh.':
    'Antigravity 登录已过期，请打开 Antigravity 或运行 `agy` 重新登录。',
  'Antigravity credentials could not be read from secure storage.':
    '无法从系统安全存储读取 Antigravity 凭据。',
  'Antigravity credentials are invalid. Sign in again in Antigravity or `agy`.':
    'Antigravity 凭据无效，请在 Antigravity 或 `agy` 中重新登录。',
  'Antigravity usage is temporarily unavailable. Try again shortly.':
    'Antigravity 用量暂时不可用，请稍后重试。',

  // Claude & Codex special errors
  'Claude OAuth settings contain an invalid URL.': 'Claude OAuth 配置包含无效的 URL。',
  'Claude login changed during refresh. Refresh again.':
    '刷新期间 Claude 登录状态发生变化，请重新刷新。',
  'The Claude account changed while OpenQuota was running. Restart OpenQuota to reconnect it safely.':
    'OpenQuota 运行期间检测到 Claude 账号变更，请重启 OpenQuota 以安全重连。',
  'Claude account settings could not be loaded.': '无法加载 Claude 账号设置。',
  'Codex credentials changed while refreshing. Run `codex` to sign in again.':
    '刷新期间 Codex 凭据发生变化，请运行 `codex` 重新登录。',
  'The Codex account changed while usage was refreshing. Refresh again.':
    '刷新期间 Codex 账号发生变化，请重新刷新。',

  // Copilot, Cursor, Devin, OpenCode, MiniMax, Z.ai
  'Sign in to GitHub Copilot in your editor, or run `gh auth login`, and try again.':
    '请在编辑器中登录 GitHub Copilot 或运行 `gh auth login` 后重试。',
  'Your GitHub token is invalid or expired. Run `gh auth login` and try again.':
    'GitHub 令牌无效或已过期，请运行 `gh auth login` 后重试。',
  'Not logged in. Sign in via Cursor app or run `agent login`.':
    '尚未登录，请通过 Cursor 客户端登录或运行 `agent login`。',
  'Session expired. Sign in via Cursor app or run `agent login`.':
    '会话已过期，请通过 Cursor 客户端登录或运行 `agent login`。',
  'Token expired. Sign in via Cursor app or run `agent login`.':
    '令牌已过期，请通过 Cursor 客户端登录或运行 `agent login`。',
  'Usage request failed after refresh. Try again.': '刷新凭据后用量请求仍失败，请重试。',
  'Total usage limit missing from API response.': '接口返回缺少总用量上限字段。',
  'No active Cursor subscription.': '当前没有生效的 Cursor 订阅。',
  'Devin is not logged in. Run `devin auth login` or sign in to the Devin app.':
    'Devin 尚未登录，请运行 `devin auth login` 或在 Devin 客户端登录。',
  'Devin login expired. Run `devin auth login` or sign in to the Devin app.':
    'Devin 登录已过期，请运行 `devin auth login` 或在 Devin 客户端重新登录。',
  'OpenCode was not detected. Sign in to OpenCode Go or use OpenCode locally first.':
    '未检测到 OpenCode，请先登录 OpenCode Go 或在本地使用一次 OpenCode。',
  'OpenCode login data could not be read. Sign in to OpenCode Go again.':
    '无法读取 OpenCode 登录数据，请重新登录 OpenCode Go。',
  'The OpenCode data directory could not be read.': '无法读取 OpenCode 数据目录。',
  'OpenCode local usage data is temporarily unavailable.': 'OpenCode 本地用量数据暂时不可用。',
  'OpenCode Go login data is invalid or expired. Sign in to OpenCode Go again.':
    'OpenCode Go 登录信息无效或已过期，请重新登录。',
  'OpenCode Go subscription required.': '需要有效的 OpenCode Go 订阅。',
  'No active MiniMax token plan. Subscribe at minimax.io to view usage.':
    '当前没有生效的 MiniMax Token Plan，请在 minimax.io 订阅后查看。',
  'No active GLM Coding Plan. Subscribe at z.ai/subscribe to view usage.':
    '当前没有生效的 GLM Coding Plan，请在 z.ai/subscribe 订阅后查看。',

  // Service & settings errors
  'OpenQuota cache is unavailable.': 'OpenQuota 本地缓存不可用。',
  'Unknown provider.': '未知的服务提供方。',
  'Provider refresh timed out.': '服务用量刷新超时。',
  'Provider refresh stopped unexpectedly.': '服务刷新任务意外中止。',
  'Provider refresh is temporarily unavailable.': '服务刷新暂时不可用。',
  'That provider does not accept an API key.': '该服务不支持配置 API 密钥。',
  'That global shortcut is invalid or already in use.': '该全局快捷键无效或已被占用。',
  'Launch at login status could not be read.': '无法读取开机自启动状态。',
  'Launch at login could not be updated.': '无法更新开机自启动设置。',
  'The saved global shortcut is currently unavailable.': '已保存的全局快捷键当前不可用。',
  'Settings changed before they could be saved. Please try again.':
    '设置在保存前发生了变更，请重试。',
  'OpenQuota settings could not be saved.': '无法保存 OpenQuota 设置。',
  'OpenQuota settings are temporarily unavailable.': 'OpenQuota 设置暂时不可用。',
  'OpenQuota window is unavailable.': 'OpenQuota 窗口当前不可用。',
  'Update source must be a valid HTTPS URL.': '更新源必须是有效的 HTTPS 地址。',
};

const BACKEND_PATTERNS_ZH: Array<[RegExp, (...groups: string[]) => string]> = [
  [
    /^Not logged in\. Run `([^`]+)` to authenticate\.$/,
    (_, cmd) => `尚未登录，请运行 \`${cmd}\` 完成认证。`,
  ],
  [
    /^(.+) is not logged in\. Run `([^`]+)`\.$/,
    (_, name, cmd) => `${name} 尚未登录，请运行 \`${cmd}\`。`,
  ],
  [
    /^Your (.+) (session|token|access token) expired\. Run `([^`]+)` to sign in again\.$/,
    (_, name, kind, cmd) =>
      `${name} ${kind === 'session' ? '会话' : '令牌'}已过期，请运行 \`${cmd}\` 重新登录。`,
  ],
  [
    /^Your (.+) session was revoked\. Run `([^`]+)` to sign in again\.$/,
    (_, name, cmd) => `${name} 会话已被撤销，请运行 \`${cmd}\` 重新登录。`,
  ],
  [
    /^(.+) login expired\. Run `([^`]+)` again\.$/,
    (_, name, cmd) => `${name} 登录已过期，请重新运行 \`${cmd}\`。`,
  ],
  [
    /^(.+) (login|auth) data is invalid\. Run `([^`]+)`(?: again| to sign in again)\.$/,
    (_, name, _kind, cmd) => `${name} 登录数据无效，请运行 \`${cmd}\` 重新登录。`,
  ],
  [
    /^(?:Refreshed|The refreshed) (.+) (?:credentials|login) could not be saved\.$/,
    (_, name) => `无法保存刷新后的 ${name} 登录凭据。`,
  ],
  [
    /^Could not (?:connect to|reach) (.+)\. Check your internet connection\.$/,
    (_, name) => `无法连接到 ${name}，请检查网络连接。`,
  ],
  [
    /^(.+?) (?:usage |billing )?request failed \(HTTP (\d+)\)\.$/,
    (_, name, status) => `${name} 请求失败（HTTP ${status}）。`,
  ],
  [
    /^(.+) returned an invalid (?:usage|billing) response\.$/,
    (_, name) => `${name} 返回了无效的用量响应。`,
  ],
  [/^(.+) usage data is temporarily unavailable\.$/, (_, name) => `${name} 用量数据暂时不可用。`],
  [
    /^(.+) (?:usage|quota) data is unavailable for this account\.$/,
    (_, name) => `当前账号无法获取 ${name} 用量数据。`,
  ],
  [
    /^Local (.+) usage logs could not be processed\.$/,
    (_, name) => `无法解析本地 ${name} 用量日志。`,
  ],
  [
    /^Add an? (.+) API key in Customize or set ([A-Z0-9_]+)\.$/,
    (_, name, envVar) => `请在「自定义」中添加 ${name} API 密钥，或设置环境变量 ${envVar}。`,
  ],
  [
    /^Add an? (.+) API key in Customize to view usage\.$/,
    (_, name) => `请在「自定义」中添加 ${name} API 密钥以查看用量。`,
  ],
  [
    /^The (.+) API key is invalid\. Check it (?:in|at) (.+)\.$/,
    (_, name, place) => `${name} API 密钥无效，请在 ${place} 检查。`,
  ],
  [
    /^The (.+) API key could not be read or updated\.$/,
    (_, name) => `无法读取或更新 ${name} API 密钥。`,
  ],
  [
    /^From your (.+) (usage history|logs) \(estimated\)$/,
    (_, provider, source) =>
      source === 'logs'
        ? `来自你的 ${provider} 本地日志（估算）`
        : `来自你的 ${provider} 用量历史（估算）`,
  ],
  [/^From your (.+) usage history$/, (_, provider) => `来自你的 ${provider} 用量历史`],
];

const KNOWN_UPDATE_ERROR_CODES = new Set([
  'busy',
  'not_configured',
  'up_to_date',
  'manual_install_required',
  'download_forbidden',
  'rate_limited',
  'signature_invalid',
  'network',
  'invalid_endpoint',
  'update_failed',
]);

function detectSystemLocale(): Locale {
  if (typeof navigator !== 'undefined' && typeof navigator.language === 'string') {
    if (navigator.language.toLowerCase().startsWith('zh')) return 'zh-CN';
  }
  return 'en';
}

let currentPreference = $state<LanguagePreference>('system');

export const i18n = {
  get preference(): LanguagePreference {
    return currentPreference;
  },
  get locale(): Locale {
    if (currentPreference === 'zhCn') return 'zh-CN';
    if (currentPreference === 'en') return 'en';
    return detectSystemLocale();
  },
  setPreference(next: LanguagePreference) {
    currentPreference = next;
  },
};

export const dictionaries = { en, 'zh-CN': zhCN } as const;
export const sharedDictionary = sharedI18n;

export function t(key: MessageKey, params?: Record<string, string | number>): string {
  const dict = i18n.locale === 'zh-CN' ? zhCN : en;
  let value = dict[key] ?? en[key];
  if (params) {
    for (const [paramKey, paramValue] of Object.entries(params)) {
      value = value.replaceAll(`{${paramKey}}`, String(paramValue));
    }
  }
  return value;
}

export function translateMetricLabel(label: string): string {
  if (i18n.locale !== 'zh-CN') return label;
  return METRIC_LABELS_ZH[label] ?? label;
}

export function translateLinkLabel(label: string): string {
  if (i18n.locale !== 'zh-CN') return label;
  return LINK_LABELS_ZH[label] ?? label;
}

export function translateUnit(unit: string): string {
  if (i18n.locale !== 'zh-CN') return unit;
  return UNITS_ZH[unit.trim().toLowerCase()] ?? unit;
}

export function translateUpdateFailure(failure: UpdateFailure): UpdateFailure {
  if (i18n.locale !== 'zh-CN') return failure;
  if (!KNOWN_UPDATE_ERROR_CODES.has(failure.code)) {
    return {
      ...failure,
      message: translateBackendText(failure.message),
      action: translateBackendText(failure.action),
    };
  }
  const messageKey = `updateError.${failure.code}.message` as MessageKey;
  const actionKey = `updateError.${failure.code}.action` as MessageKey;
  return {
    ...failure,
    message: t(messageKey),
    action: t(actionKey),
  };
}

export function translateBackendText(text: string): string {
  if (i18n.locale !== 'zh-CN') return text;
  const exact = BACKEND_MESSAGES_ZH[text];
  if (exact) return exact;
  for (const [pattern, format] of BACKEND_PATTERNS_ZH) {
    const match = text.match(pattern);
    if (match) return format(...match);
  }
  return text;
}
