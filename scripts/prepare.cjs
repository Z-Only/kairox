const { existsSync } = require('node:fs');
const { spawnSync } = require('node:child_process');
const path = require('node:path');

const isCi = !!process.env.CI || process.env.NODE_ENV === 'production';
if (isCi) {
  process.exit(0);
}

const huskyBin = process.platform === 'win32'
  ? path.join(process.cwd(), 'node_modules', '.bin', 'husky.cmd')
  : path.join(process.cwd(), 'node_modules', '.bin', 'husky');

if (!existsSync(huskyBin)) {
  process.exit(0);
}

const result = spawnSync(huskyBin, [], { stdio: 'inherit', shell: false });
process.exit(result.status ?? 0);
