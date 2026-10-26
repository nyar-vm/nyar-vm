#!/usr/bin/env node
import {runLegionCli} from '../dist/cli/legion-cli.js';

const code = await runLegionCli(process.argv.slice(2));
process.exit(code ?? 0);
