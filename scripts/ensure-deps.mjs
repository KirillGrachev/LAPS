import { existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * npm не ставит node_modules сам по себе, а исходники передаются без него.
 * Поэтому перед dev/build проверяем, что установлены именно те пакеты,
 * которые нужны командам: vite и Tauri CLI. Если их нет — один раз
 * выполняем npm ci и продолжаем.
 */
const root = dirname(dirname(fileURLToPath(import.meta.url)));
const required = [
  join(root, 'node_modules', 'vite', 'bin', 'vite.js'),
  join(root, 'node_modules', '@tauri-apps', 'cli', 'package.json'),
];

if (required.every((path) => existsSync(path))) {
  process.exit(0);
}

const lockfile = join(root, 'package-lock.json');
const args = existsSync(lockfile) ? ['ci'] : ['install'];
const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm';

console.log('Зависимости не установлены — выполняю npm ' + args.join(' ') + '…');
const result = spawnSync(npm, args, {
  cwd: root,
  stdio: 'inherit',
  shell: process.platform === 'win32',
});

if (result.error) {
  console.error('Не удалось запустить npm: ' + result.error.message);
  process.exit(1);
}
process.exit(result.status ?? 1);
