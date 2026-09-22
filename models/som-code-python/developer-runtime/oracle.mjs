// Only run the project's authored training cases. Never execute user requests.
import fs from 'node:fs';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import { JSDOM } from 'jsdom';

const cases = JSON.parse(fs.readFileSync(0, 'utf8'));
const results = [];
for (const item of cases) {
  const passed = [];
  for (const source of item.sources) {
    const dom = item.dom ? new JSDOM('<!doctype html><body></body>') : null;
    const context = vm.createContext({ assert, document: dom?.window.document,
      Event: dom?.window.Event, URL, URLSearchParams, structuredClone });
    try {
      // No network, process, require, or filesystem bindings are provided.
      const script = new vm.Script(`(async () => {\n${source}\n${item.checks}\n})()`);
      await script.runInContext(context, { timeout: 500 });
      passed.push(true);
    } catch (error) {
      passed.push(false);
    } finally {
      dom?.window.close();
    }
  }
  results.push({ id: item.id, passed });
}
process.stdout.write(JSON.stringify(results));
