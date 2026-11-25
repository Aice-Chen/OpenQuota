import fs from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => fs.readFileSync(new URL(path, root), 'utf8');
const ci = read('.github/workflows/ci.yml');
const release = read('.github/workflows/build-windows.yml');
const action = read('.github/actions/platform-smoke/action.yml');
const windows = read('.github/actions/platform-smoke/scripts/windows.ps1');
const tauriConf = JSON.parse(read('src-tauri/tauri.conf.json'));
const readme = read('README.md');
const releasing = read('docs/releasing.md');

const requireContracts = (source, content, contracts) => {
  for (const contract of contracts) {
    if (!content.includes(contract)) {
      throw new Error(`${source} configuration is missing: ${contract}`);
    }
  }
};

requireContracts('CI', ci, [
  'runs-on: windows-latest',
  'pnpm verify:versions',
  'pnpm verify:contracts',
  'pnpm lint',
  'pnpm check',
  'pnpm test',
  'cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check',
  'cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings',
  'cargo test --manifest-path src-tauri/Cargo.toml --all-targets',
  'Test Windows Credential Manager integration',
  'pnpm build:installer',
  'uses: ./.github/actions/platform-smoke',
]);

requireContracts('Windows release workflow', release, [
  'runs-on: windows-latest',
  'TAURI_SIGNING_PRIVATE_KEY',
  'tauri.release.conf.json',
  'uses: ./.github/actions/platform-smoke',
  'node .github/scripts/create-updater-json.mjs',
  'gh release create',
  'gh release upload',
]);

requireContracts('Windows smoke action', action, [
  'Install, start, and uninstall the Windows NSIS package',
  'artifact-root:',
  'scripts/windows.ps1',
]);

requireContracts('Windows package smoke', windows, [
  '*-setup.exe',
  'RUNNER_TEMP is required for the Windows installer smoke test',
  'Refusing to disturb an existing OpenQuota installation',
  '@(\'/S\', "/D=$installRoot")',
  'system tray integration ready',
  'OpenQuota startup completed',
  'for ($attempt = 0; $attempt -lt 60; $attempt++)',
  'Expected Windows GUI subsystem (2)',
  "-ArgumentList '/S'",
  'remained installed after the NSIS uninstall smoke test',
]);

if (tauriConf.identifier !== 'io.github.deviffyy.openquota') {
  throw new Error('Unexpected bundle identifier; preserving it keeps local user data intact.');
}
if (!tauriConf.plugins?.updater?.pubkey) {
  throw new Error('Tauri updater public key is missing from src-tauri/tauri.conf.json.');
}
if (
  !Array.isArray(tauriConf.plugins?.updater?.endpoints) ||
  !tauriConf.plugins.updater.endpoints.some((endpoint) =>
    endpoint.includes('Aice-Chen/OpenQuota/releases/latest/download/latest.json'),
  )
) {
  throw new Error('Tauri updater endpoint must point to Aice-Chen/OpenQuota latest.json.');
}

requireContracts('release documentation', releasing, [
  'TAURI_SIGNING_PRIVATE_KEY',
  'pnpm bump',
  'latest.json',
]);

requireContracts('README', readme, ['Windows', 'Aice-Chen/OpenQuota', 'MIT']);

console.log(
  'Windows CI, release workflow, NSIS smoke test, and Minisign updater contracts verified.',
);
