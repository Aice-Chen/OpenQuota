import fs from 'node:fs';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const nextVersion = process.argv[2]?.trim();
if (!nextVersion || !/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(nextVersion)) {
  console.error('Usage: pnpm bump <major.minor.patch> (e.g. pnpm bump 0.5.1)');
  process.exit(1);
}

const root = new URL('../', import.meta.url);
const readText = (relPath) => fs.readFileSync(new URL(relPath, root), 'utf8');
const writeText = (relPath, content) => fs.writeFileSync(new URL(relPath, root), content);

const packageJson = JSON.parse(readText('package.json'));
packageJson.version = nextVersion;
writeText('package.json', `${JSON.stringify(packageJson, null, 2)}\n`);

const tauriConfig = JSON.parse(readText('src-tauri/tauri.conf.json'));
tauriConfig.version = nextVersion;
writeText('src-tauri/tauri.conf.json', `${JSON.stringify(tauriConfig, null, 2)}\n`);

const cargoToml = readText('src-tauri/Cargo.toml');
const updatedCargoToml = cargoToml.replace(
  /(\[package\][\s\S]*?^version\s*=\s*")[^"]+(")/m,
  `$1${nextVersion}$2`,
);
if (updatedCargoToml === cargoToml && !cargoToml.includes(`version = "${nextVersion}"`)) {
  throw new Error('Could not update package version in src-tauri/Cargo.toml.');
}
writeText('src-tauri/Cargo.toml', updatedCargoToml);

const cargoLock = readText('src-tauri/Cargo.lock');
const updatedCargoLock = cargoLock.replace(
  /(name = "openquota"\r?\nversion = ")[^"]+(")/,
  `$1${nextVersion}$2`,
);
if (
  updatedCargoLock === cargoLock &&
  !cargoLock.includes(`name = "openquota"\nversion = "${nextVersion}"`)
) {
  throw new Error('Could not update openquota version in src-tauri/Cargo.lock.');
}
writeText('src-tauri/Cargo.lock', updatedCargoLock);

execFileSync(
  process.execPath,
  [fileURLToPath(new URL('scripts/verify/verify-versions.js', root))],
  {
    stdio: 'inherit',
  },
);
console.log(
  `Updated package.json, src-tauri/tauri.conf.json, src-tauri/Cargo.toml, and src-tauri/Cargo.lock to ${nextVersion}.`,
);
