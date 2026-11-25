<script lang="ts">
  import { tick } from 'svelte';
  import { addProviderAccount, removeProviderAccount } from './backend';
  import Icon from './Icon.svelte';
  import { t, translateBackendText } from './i18n.svelte';
  import ProviderIcon from './ProviderIcon.svelte';
  import type { BootstrapState } from './types';

  interface Props {
    providerId: string;
    providerName: string;
    canAddAccount: boolean;
    canRemoveAccount: boolean;
    onAccountAdded?: (bootstrap: BootstrapState) => void;
    onAccountRemoved?: (bootstrap: BootstrapState, removedId: string) => void;
  }

  let {
    providerId,
    providerName,
    canAddAccount,
    canRemoveAccount,
    onAccountAdded,
    onAccountRemoved,
  }: Props = $props();

  let open = $state(false);
  let label = $state('');
  let credential = $state('');
  let revealCredential = $state(false);
  let pendingAction = $state<'import' | 'paste' | 'remove' | null>(null);
  let confirmingRemoval = $state(false);
  let error = $state<string | null>(null);
  let editorToggle = $state<HTMLButtonElement>();
  let removeTrigger = $state<HTMLButtonElement>();
  let removeCancelButton = $state<HTMLButtonElement>();

  function errorMessage(cause: unknown, fallback: string) {
    if (typeof cause === 'string') return translateBackendText(cause);
    if (cause instanceof Error && cause.message) return translateBackendText(cause.message);
    return fallback;
  }

  function resetAddEditor() {
    label = '';
    credential = '';
    revealCredential = false;
    error = null;
  }

  function toggleOpen() {
    open = !open;
    if (open) resetAddEditor();
  }

  async function importCurrentAccount() {
    if (pendingAction) return;
    pendingAction = 'import';
    error = null;
    try {
      const bootstrap = await addProviderAccount(providerId, label.trim() || null, null);
      resetAddEditor();
      open = false;
      onAccountAdded?.(bootstrap);
      await tick();
      editorToggle?.focus();
    } catch (cause) {
      error = errorMessage(cause, t('accounts.addFailed'));
    } finally {
      pendingAction = null;
    }
  }

  async function addPastedAccount() {
    const trimmedCredential = credential.trim();
    if (!trimmedCredential || pendingAction) return;
    pendingAction = 'paste';
    error = null;
    try {
      const bootstrap = await addProviderAccount(
        providerId,
        label.trim() || null,
        trimmedCredential,
      );
      resetAddEditor();
      open = false;
      onAccountAdded?.(bootstrap);
      await tick();
      editorToggle?.focus();
    } catch (cause) {
      error = errorMessage(cause, t('accounts.addFailed'));
    } finally {
      pendingAction = null;
    }
  }

  async function requestRemoval() {
    if (pendingAction || confirmingRemoval) return;
    confirmingRemoval = true;
    error = null;
    await tick();
    removeCancelButton?.focus();
  }

  async function cancelRemoval() {
    if (pendingAction) return;
    confirmingRemoval = false;
    await tick();
    removeTrigger?.focus();
  }

  function handleRemovalKeydown(event: KeyboardEvent) {
    if (event.key !== 'Escape' || pendingAction) return;
    event.preventDefault();
    event.stopPropagation();
    void cancelRemoval();
  }

  async function confirmRemove() {
    if (pendingAction) return;
    pendingAction = 'remove';
    error = null;
    try {
      const bootstrap = await removeProviderAccount(providerId);
      confirmingRemoval = false;
      onAccountRemoved?.(bootstrap, providerId);
    } catch (cause) {
      error = errorMessage(cause, t('accounts.removeFailed'));
    } finally {
      pendingAction = null;
    }
  }
</script>

{#if canAddAccount}
  <section class="accounts-section" aria-label={t('accounts.sectionAria', { name: providerName })}>
    <h2>{t('accounts.heading')}</h2>
    <div class="accounts-card">
      <div class="accounts-summary">
        <ProviderIcon {providerId} size={18} />
        <div class="accounts-summary-text">
          <span class="accounts-title">{t('accounts.addTitle')}</span>
          <small>{t('accounts.addSubtitle')}</small>
        </div>
        <button bind:this={editorToggle} type="button" onclick={toggleOpen}
          >{open ? t('common.done') : t('common.add')}</button
        >
      </div>
      {#if open}
        <div class="accounts-editor">
          <label class="accounts-field">
            <span>{t('accounts.labelLabel')}</span>
            <input
              type="text"
              maxlength="48"
              bind:value={label}
              autocomplete="off"
              placeholder={t('accounts.labelPlaceholder')}
              disabled={pendingAction !== null}
            />
          </label>
          <div class="accounts-actions">
            <button
              class="primary"
              type="button"
              disabled={pendingAction !== null}
              onclick={() => void importCurrentAccount()}
              >{pendingAction === 'import'
                ? t('accounts.importing')
                : t('accounts.importCurrent')}</button
            >
          </div>
          <div class="accounts-divider"><span>{t('accounts.orPasteToken')}</span></div>
          <div class="accounts-credential-input">
            <input
              type={revealCredential ? 'text' : 'password'}
              bind:value={credential}
              autocomplete="off"
              spellcheck="false"
              placeholder={t('accounts.credentialPlaceholder')}
              aria-label={t('accounts.credentialAria', { name: providerName })}
              disabled={pendingAction !== null}
            />
            <button
              class="field-icon"
              type="button"
              aria-label={revealCredential
                ? t('accounts.hideCredential')
                : t('accounts.showCredential')}
              onclick={() => (revealCredential = !revealCredential)}
            >
              <Icon name={revealCredential ? 'eye-off' : 'eye'} size={15} />
            </button>
          </div>
          <div class="accounts-actions">
            <button
              type="button"
              disabled={!credential.trim() || pendingAction !== null}
              onclick={() => void addPastedAccount()}
              >{pendingAction === 'paste'
                ? t('accounts.adding')
                : t('accounts.addWithCredential')}</button
            >
          </div>
          {#if error}<div class="accounts-error" role="alert">{error}</div>{/if}
        </div>
      {/if}
    </div>
  </section>
{/if}

{#if canRemoveAccount}
  <section class="accounts-section" aria-label={t('accounts.sectionAria', { name: providerName })}>
    <h2>{t('accounts.removeHeading')}</h2>
    <div class="accounts-card">
      <div class="accounts-summary">
        <ProviderIcon {providerId} size={18} />
        <span class="accounts-title">{providerName}</span>
        <button
          bind:this={removeTrigger}
          class="danger-trigger"
          type="button"
          disabled={pendingAction !== null || confirmingRemoval}
          aria-expanded={confirmingRemoval}
          onclick={() => void requestRemoval()}>{t('accounts.removeButton')}</button
        >
      </div>
      {#if confirmingRemoval}
        <div
          class="accounts-remove-confirm"
          role="group"
          aria-labelledby={`remove-account-title-${providerId}`}
          aria-describedby={`remove-account-message-${providerId}`}
        >
          <strong id={`remove-account-title-${providerId}`}
            >{t('accounts.removeConfirmTitle', { name: providerName })}</strong
          >
          <span id={`remove-account-message-${providerId}`}
            >{t('accounts.removeConfirmMessage')}</span
          >
          <div class="accounts-remove-actions">
            <button
              bind:this={removeCancelButton}
              type="button"
              disabled={pendingAction !== null}
              onkeydown={handleRemovalKeydown}
              onclick={() => void cancelRemoval()}>{t('common.cancel')}</button
            >
            <button
              class="destructive"
              type="button"
              disabled={pendingAction !== null}
              onkeydown={handleRemovalKeydown}
              onclick={() => void confirmRemove()}
              >{pendingAction === 'remove'
                ? t('accounts.removing')
                : t('accounts.removeButton')}</button
            >
          </div>
          {#if error}<div class="accounts-error" role="alert">{error}</div>{/if}
        </div>
      {/if}
    </div>
  </section>
{/if}

<style>
  .accounts-section {
    margin-bottom: 14px;
  }

  .accounts-section h2 {
    margin: 0 8px 5px;
    color: var(--secondary);
    font-size: 11px;
    font-weight: 600;
  }

  .accounts-card {
    overflow: hidden;
    border-radius: 12px;
    background: var(--card);
  }

  .accounts-summary {
    display: flex;
    min-height: 44px;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
  }

  .accounts-summary-text {
    display: flex;
    min-width: 0;
    flex: 1;
    flex-direction: column;
    gap: 1px;
  }

  .accounts-title {
    min-width: 0;
    flex: 1;
    overflow: hidden;
    font-size: 12px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .accounts-summary-text small {
    color: var(--secondary);
    font-size: 10px;
    line-height: 13px;
  }

  button {
    padding: 4px 8px;
    border: 0;
    border-radius: 6px;
    color: var(--text);
    background: var(--button-hover);
    font-size: 10px;
  }

  button.danger-trigger {
    color: var(--error);
    background: color-mix(in srgb, var(--error) 10%, transparent);
    font-weight: 600;
  }

  .accounts-editor {
    display: grid;
    gap: 9px;
    padding: 11px 12px 12px;
    border-top: 1px solid var(--separator);
    background: color-mix(in srgb, var(--card) 75%, var(--tray));
  }

  .accounts-field {
    display: grid;
    gap: 4px;
  }

  .accounts-field > span {
    color: var(--secondary);
    font-size: 10px;
  }

  .accounts-field input,
  .accounts-credential-input input {
    min-width: 0;
    height: 28px;
    box-sizing: border-box;
    padding: 4px 8px;
    border: 1px solid var(--separator);
    border-radius: 7px;
    color: var(--text);
    background: var(--tray);
    font: inherit;
    font-size: 11px;
    outline: none;
  }

  .accounts-field input:focus,
  .accounts-credential-input input:focus {
    border-color: var(--meter-fill);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--meter-fill) 20%, transparent);
  }

  .accounts-divider {
    color: var(--secondary);
    font-size: 10px;
  }

  .accounts-credential-input {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .accounts-credential-input input {
    flex: 1;
  }

  .accounts-credential-input .field-icon {
    display: grid;
    width: 26px;
    height: 26px;
    padding: 0;
    color: var(--secondary);
    background: transparent;
    place-items: center;
  }

  .accounts-credential-input .field-icon:hover:not(:disabled) {
    background: var(--button-hover);
  }

  .accounts-actions {
    display: flex;
    justify-self: start;
    align-items: center;
    gap: 7px;
  }

  .accounts-actions button {
    min-height: 26px;
    padding: 5px 10px;
  }

  .accounts-actions .primary {
    color: white;
    background: var(--meter-fill);
    font-weight: 600;
  }

  .accounts-remove-confirm {
    display: grid;
    gap: 6px;
    padding: 10px 12px 12px;
    border-top: 1px solid var(--separator);
    background: color-mix(in srgb, var(--meter-critical) 8%, transparent);
    font-size: 10px;
  }

  .accounts-remove-confirm > span {
    color: var(--secondary);
    line-height: 14px;
  }

  .accounts-remove-actions {
    display: flex;
    gap: 7px;
  }

  .accounts-remove-actions button {
    min-height: 26px;
    padding: 5px 10px;
  }

  .accounts-remove-actions .destructive {
    color: var(--tray);
    background: var(--error);
    font-weight: 600;
  }

  .accounts-error {
    padding: 6px 7px;
    border-radius: 7px;
    color: var(--error);
    background: var(--error-bg);
    font-size: 10px;
  }
</style>
