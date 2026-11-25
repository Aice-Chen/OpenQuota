import { fireEvent, render, waitFor } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ProviderCatalogIndex } from './metrics';
import { i18n, translateBackendText } from './i18n.svelte';
import ProviderAccountsSection from './ProviderAccountsSection.svelte';
import type { BootstrapState, ProviderCatalog } from './types';

const backendMocks = vi.hoisted(() => ({
  addProviderAccount: vi.fn(),
  removeProviderAccount: vi.fn(),
}));

vi.mock('./backend', async () => {
  const actual = await vi.importActual<typeof import('./backend')>('./backend');
  return {
    ...actual,
    addProviderAccount: backendMocks.addProviderAccount,
    removeProviderAccount: backendMocks.removeProviderAccount,
  };
});

function sampleCatalog(): ProviderCatalog {
  return {
    providers: [
      {
        id: 'antigravity',
        displayName: 'Antigravity',
        shortName: 'A',
        fallbackEnabled: false,
        dynamicQuotas: true,
        localUsageSourceNote: null,
        links: [],
        metrics: [],
      },
      {
        id: 'antigravity@1234abcd',
        displayName: 'Antigravity',
        shortName: 'A',
        fallbackEnabled: false,
        dynamicQuotas: true,
        localUsageSourceNote: null,
        links: [],
        metrics: [],
      },
    ],
    apiKeyProviderIds: [],
    multiAccountProviderIds: ['antigravity'],
    removableProviderIds: ['antigravity@1234abcd'],
  };
}

describe('M3 multi-account catalog capabilities and UI', () => {
  beforeEach(() => {
    backendMocks.addProviderAccount.mockReset();
    backendMocks.removeProviderAccount.mockReset();
    i18n.setPreference('en');
  });

  it('indexes multiAccountProviderIds and removableProviderIds without hardcoding', () => {
    const index = new ProviderCatalogIndex(sampleCatalog());
    expect(index.supportsMultiAccount('antigravity')).toBe(true);
    expect(index.isRemovable('antigravity')).toBe(false);
    expect(index.supportsMultiAccount('antigravity@1234abcd')).toBe(false);
    expect(index.isRemovable('antigravity@1234abcd')).toBe(true);
  });

  it('imports current sign-in or pasted credentials and invokes onAccountAdded', async () => {
    const fakeBootstrap = {
      catalog: sampleCatalog(),
      usage: { providers: {} },
      settings: {} as BootstrapState['settings'],
    } as BootstrapState;
    backendMocks.addProviderAccount.mockResolvedValue(fakeBootstrap);
    const onAccountAdded = vi.fn();

    const { getByRole, getByPlaceholderText } = render(ProviderAccountsSection, {
      providerId: 'antigravity',
      providerName: 'Antigravity',
      canAddAccount: true,
      canRemoveAccount: false,
      onAccountAdded,
    });

    await fireEvent.click(getByRole('button', { name: 'Add' }));
    await fireEvent.input(getByPlaceholderText('e.g. Work or Personal'), {
      target: { value: 'Work' },
    });
    await fireEvent.click(getByRole('button', { name: 'Import Current Sign-In' }));

    await waitFor(() => {
      expect(backendMocks.addProviderAccount).toHaveBeenCalledWith('antigravity', 'Work', null);
      expect(onAccountAdded).toHaveBeenCalledWith(fakeBootstrap);
    });
  });

  it('confirms before removing an added account card and invokes onAccountRemoved', async () => {
    const fakeBootstrap = {
      catalog: {
        ...sampleCatalog(),
        providers: [sampleCatalog().providers[0]],
        removableProviderIds: [],
      },
      usage: { providers: {} },
      settings: {} as BootstrapState['settings'],
    } as BootstrapState;
    backendMocks.removeProviderAccount.mockResolvedValue(fakeBootstrap);
    const onAccountRemoved = vi.fn();

    const { getByRole, getAllByRole } = render(ProviderAccountsSection, {
      providerId: 'antigravity@1234abcd',
      providerName: 'Work',
      canAddAccount: false,
      canRemoveAccount: true,
      onAccountRemoved,
    });

    await fireEvent.click(getByRole('button', { name: 'Remove Account' }));
    expect(backendMocks.removeProviderAccount).not.toHaveBeenCalled();

    const buttons = getAllByRole('button', { name: 'Remove Account' });
    await fireEvent.click(buttons[buttons.length - 1]);

    await waitFor(() => {
      expect(backendMocks.removeProviderAccount).toHaveBeenCalledWith('antigravity@1234abcd');
      expect(onAccountRemoved).toHaveBeenCalledWith(fakeBootstrap, 'antigravity@1234abcd');
    });
  });

  it('translates M3 multi-account backend errors in zh-CN', () => {
    i18n.setPreference('zhCn');
    expect(
      translateBackendText(
        'That account is already added. Sign in to another account first or paste different credentials.',
      ),
    ).toContain('已添加');
    expect(
      translateBackendText(
        'The pasted credential is invalid. Paste a Google refresh token (1//...) or valid Antigravity credential JSON.',
      ),
    ).toContain('凭据格式无效');
  });
});
