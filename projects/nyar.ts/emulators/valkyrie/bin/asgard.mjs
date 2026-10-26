#!/usr/bin/env node
import {runAsgardCli} from '../dist/cli/asgard-cli.js';

const code = await runAsgardCli(process.argv.slice(2));
process.exit(code ?? 0);
