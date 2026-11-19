import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const windows = process.platform === 'win32';
const command = windows ? process.env.ComSpec || 'cmd.exe' : './gradlew';
const args = windows ? ['/d', '/c', 'gradlew.bat', ':app:assembleRelease'] : [':app:assembleRelease'];
const result = spawnSync(command, args, {
  cwd: fileURLToPath(new URL('../android/', import.meta.url)),
  stdio: 'inherit',
});
if (result.error) throw result.error;
process.exitCode = result.status ?? 1;
