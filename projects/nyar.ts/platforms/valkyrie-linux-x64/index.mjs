import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const binding = require(join(dirname(fileURLToPath(import.meta.url)), 'lib', 'valkyrie.linux-x64-gnu.node'));
export default binding;
