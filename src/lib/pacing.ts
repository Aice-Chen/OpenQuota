import { i18n, t } from './i18n.svelte';
import type { QuotaWindow } from './types';

export type PaceSeverity = 'level' | 'healthy' | 'close' | 'runningOut' | 'spent';

export interface PaceProjection {
  severity: PaceSeverity;
  projectedUsedPercent: number | null;
  evenPacePercent: number | null;
  runOutAt: number | null;
}

export function projectPace(window: QuotaWindow, now: number): PaceProjection {
  const used = clamp(window.usedPercent, 0, 100);
  if (isVisiblySpent(window, used)) {
    return { severity: 'spent', projectedUsedPercent: 100, evenPacePercent: null, runOutAt: now };
  }
  if (used <= 0) return level();
  const reset = window.resetsAt ? new Date(window.resetsAt).getTime() : Number.NaN;
  if (!Number.isFinite(reset) || reset <= now || window.periodSeconds <= 0) return level();
  const periodMs = window.periodSeconds * 1000;
  const start = reset - periodMs;
  const elapsed = Math.max(0, now - start);
  const progress = clamp(elapsed / periodMs, 0, 1);
  if (elapsed < Math.max(60_000, periodMs * 0.01)) return level();
  const projected = used / progress;
  if (projected <= 90) {
    return {
      severity: 'healthy',
      projectedUsedPercent: projected,
      evenPacePercent: progress * 100,
      runOutAt: null,
    };
  }
  if (used < 5) return level();
  if (projected <= 100) {
    const spare = Math.round(100 - projected);
    return {
      severity: spare >= 1 ? 'close' : 'runningOut',
      projectedUsedPercent: projected,
      evenPacePercent: progress * 100,
      runOutAt: null,
    };
  }
  const candidate = start + (elapsed * 100) / used;
  return {
    severity: 'runningOut',
    projectedUsedPercent: projected,
    evenPacePercent: progress * 100,
    runOutAt: candidate > now && candidate < reset ? candidate : null,
  };
}

export function isFreshSessionWindow(window: QuotaWindow, now: number, isSessionWindow: boolean) {
  if (!isSessionWindow || window.usedPercent > 0 || !window.resetsAt) return false;
  const reset = new Date(window.resetsAt).getTime();
  return Number.isFinite(reset) && now < reset;
}

export function paceTooltip(value: PaceProjection) {
  if (value.severity === 'level') return null;
  if (value.severity === 'spent') return t('quota.limitReached');
  const projected = value.projectedUsedPercent;
  if (projected === null) return null;
  if (value.severity === 'healthy') {
    return t('pace.leftAtReset', { percent: Math.round(100 - projected) });
  }
  if (value.severity === 'close') {
    return t('pace.usedAtReset', { percent: Math.round(projected) });
  }
  if (projected <= 100) return t('pace.fullAtReset');
  return t('pace.overAtReset', { percent: Math.max(1, Math.round(projected - 100)) });
}

type TimeFormat = 'system' | 'twelveHour' | 'twentyFourHour';

export function formatReset(
  value: string | null,
  now: number,
  mode: 'countdown' | 'exact',
  timeFormat: TimeFormat = 'system',
) {
  if (!value) return t('quota.resetUnavailable');
  const reset = new Date(value).getTime();
  if (!Number.isFinite(reset)) return t('quota.resetUnavailable');
  return formatDeadline('resets', reset, now, mode, timeFormat);
}

export function formatLimit(
  value: number | null,
  now: number,
  mode: 'countdown' | 'exact',
  timeFormat: TimeFormat = 'system',
) {
  if (value === null) return t('quota.limitReached');
  return formatDeadline('limit', value, now, mode, timeFormat);
}

export function formatExpiryParts(
  value: string,
  now: number,
  timeFormat: TimeFormat = 'system',
): { imminent: boolean; relative: string; exact: string } {
  const timestamp = new Date(value).getTime();
  if (!Number.isFinite(timestamp)) {
    const unavailable = t('quota.resetUnavailable');
    return { imminent: false, relative: unavailable, exact: unavailable };
  }
  const remaining = timestamp - now;
  const imminent = remaining <= 5 * 60_000;
  const relative = imminent ? t('reset.expiringSoon') : formatDuration(remaining);
  const exact = imminent
    ? t('reset.expiringSoon')
    : formatExactDateTime(timestamp, now, timeFormat);
  return { imminent, relative, exact };
}

function formatDeadline(
  kind: 'resets' | 'limit',
  value: number,
  now: number,
  mode: 'countdown' | 'exact',
  timeFormat: TimeFormat,
) {
  const remaining = value - now;
  if (remaining <= 0 || (mode === 'countdown' && remaining <= 5 * 60_000)) {
    return kind === 'resets' ? t('deadline.resetsSoon') : t('deadline.limitSoon');
  }
  if (mode === 'countdown') {
    const duration = formatDuration(remaining);
    return kind === 'resets'
      ? t('deadline.resetsIn', { duration })
      : t('deadline.limitIn', { duration });
  }

  const { dayDifference, monthDay, time } = exactDateParts(value, now, timeFormat);
  if (dayDifference <= 0) {
    return kind === 'resets'
      ? t('deadline.resetsToday', { time })
      : t('deadline.limitToday', { time });
  }
  if (dayDifference === 1) {
    return kind === 'resets'
      ? t('deadline.resetsTomorrow', { time })
      : t('deadline.limitTomorrow', { time });
  }
  return kind === 'resets'
    ? t('deadline.resetsDate', { date: monthDay, time })
    : t('deadline.limitDate', { date: monthDay, time });
}

function exactDateParts(value: number, now: number, timeFormat: TimeFormat) {
  const date = new Date(value);
  const current = new Date(now);
  const currentDay = Date.UTC(current.getFullYear(), current.getMonth(), current.getDate());
  const targetDay = Date.UTC(date.getFullYear(), date.getMonth(), date.getDate());
  const dayDifference = Math.round((targetDay - currentDay) / 86_400_000);
  const locale = i18n.locale === 'zh-CN' ? 'zh-CN' : undefined;
  const time = date.toLocaleTimeString(locale, {
    hour: 'numeric',
    minute: '2-digit',
    hour12: timeFormat === 'system' ? undefined : timeFormat === 'twelveHour',
  });
  const monthDay = new Intl.DateTimeFormat(locale, {
    month: 'short',
    day: 'numeric',
  }).format(date);
  return { dayDifference, monthDay, time };
}

function formatExactDateTime(value: number, now: number, timeFormat: TimeFormat) {
  const { dayDifference, monthDay, time } = exactDateParts(value, now, timeFormat);
  if (i18n.locale === 'zh-CN') {
    if (dayDifference <= 0) return `今天 ${time}`;
    if (dayDifference === 1) return `明天 ${time}`;
    return `${monthDay} ${time}`;
  }
  if (dayDifference <= 0) return `today at ${time}`;
  if (dayDifference === 1) return `tomorrow at ${time}`;
  return `${monthDay} at ${time}`;
}

function formatDuration(milliseconds: number) {
  const minutes = Math.max(1, Math.ceil(milliseconds / 60_000));
  const days = Math.floor(minutes / 1_440);
  const hours = Math.floor((minutes % 1_440) / 60);
  const remainder = minutes % 60;
  if (days > 0) return t('duration.daysHours', { days, hours });
  if (hours > 0) {
    return remainder > 0
      ? t('duration.hoursMinutes', { hours, minutes: remainder })
      : t('duration.hours', { hours });
  }
  return t('duration.minutes', { minutes: remainder });
}

function level(): PaceProjection {
  return { severity: 'level', projectedUsedPercent: null, evenPacePercent: null, runOutAt: null };
}

function isVisiblySpent(window: QuotaWindow, usedPercent: number) {
  if (
    window.format === 'dollars' &&
    window.usedValue !== null &&
    window.limitValue !== null &&
    window.limitValue > 0
  ) {
    return Math.round((window.limitValue - window.usedValue) * 100) / 100 <= 0;
  }
  return Math.round(100 - usedPercent) <= 0;
}

function clamp(value: number, minimum: number, maximum: number) {
  return Math.min(maximum, Math.max(minimum, value));
}
