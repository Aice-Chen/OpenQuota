import { describe, expect, it } from 'vitest';
import { createUpdaterMetadata } from '../../.github/scripts/create-updater-json.mjs';

describe('release updater metadata (Windows)', () => {
  const x64Asset = 'OpenQuota_0.6.0_x64-setup.exe';
  const arm64Asset = 'OpenQuota_0.6.0_arm64-setup.exe';

  const createRelease = (names: string[]) => ({
    body: 'Release notes',
    created_at: '2026-11-26T00:00:00Z',
    assets: names.flatMap((name, index) => [
      { id: index * 2 + 1, name },
      { id: index * 2 + 2, name: `${name}.sig` },
    ]),
  });

  it('creates deterministic updater metadata for Windows x64 builds', () => {
    const update = createUpdaterMetadata(
      createRelease([x64Asset]),
      { [`${x64Asset}.sig`]: `signature:${x64Asset}` },
      'Aice-Chen/OpenQuota',
      'v0.6.0',
      '0.6.0',
    );

    expect(update.version).toBe('0.6.0');
    expect(update.platforms['windows-x86_64']).toEqual({
      url: 'https://github.com/Aice-Chen/OpenQuota/releases/download/v0.6.0/OpenQuota_0.6.0_x64-setup.exe',
      signature: `signature:${x64Asset}`,
    });
    expect(update.platforms['windows-x86_64-nsis']).toEqual(update.platforms['windows-x86_64']);
    expect(Object.keys(update.platforms)).toEqual(['windows-x86_64', 'windows-x86_64-nsis']);
  });

  it('also includes Windows ARM64 when an ARM64 installer and signature are present', () => {
    const update = createUpdaterMetadata(
      createRelease([x64Asset, arm64Asset]),
      {
        [`${x64Asset}.sig`]: `signature:${x64Asset}`,
        [`${arm64Asset}.sig`]: `signature:${arm64Asset}`,
      },
      'Aice-Chen/OpenQuota',
      'v0.6.0',
      '0.6.0',
    );

    expect(update.platforms['windows-aarch64'].url).toContain('_arm64-setup.exe');
    expect(Object.keys(update.platforms)).toEqual([
      'windows-x86_64',
      'windows-x86_64-nsis',
      'windows-aarch64',
      'windows-aarch64-nsis',
    ]);
  });

  it('rejects a release missing the required Windows x64 installer', () => {
    expect(() =>
      createUpdaterMetadata(
        createRelease([arm64Asset]),
        { [`${arm64Asset}.sig`]: `signature:${arm64Asset}` },
        'Aice-Chen/OpenQuota',
        'v0.6.0',
        '0.6.0',
      ),
    ).toThrow('Expected one Windows x64 NSIS installer, found 0.');
  });

  it('rejects a release missing the updater signature file', () => {
    expect(() =>
      createUpdaterMetadata(
        {
          body: 'Release notes',
          created_at: '2026-11-26T00:00:00Z',
          assets: [{ id: 1, name: x64Asset }],
        },
        {},
        'Aice-Chen/OpenQuota',
        'v0.6.0',
        '0.6.0',
      ),
    ).toThrow(`Missing updater signature asset: ${x64Asset}.sig`);
  });
});
