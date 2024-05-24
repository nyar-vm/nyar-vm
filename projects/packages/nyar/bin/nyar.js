#!/usr/bin/env node --experimental-strip-types
import {runCli} from '../src/cli/cli.ts';

const code = await runCli(process.argv.slice(2));
process.exit(code ?? 0);
