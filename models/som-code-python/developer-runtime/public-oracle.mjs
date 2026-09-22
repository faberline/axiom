// Only pinned and inspected benchmark fixtures enter this program.
// Node vm is not a security sandbox. General prediction never calls this file.
import fs from 'node:fs';
import vm from 'node:vm';

const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const results = input.sources.map(source => {
  let script;
  try { script = new vm.Script(source + '\n' + input.tests); }
  catch (error) { return {passed:false, valid_syntax:false, error:error.name}; }
  let checks = 0;
  const console = Object.freeze({assert(value) {
    checks++;
    if (!value) throw new Error('Fixture assertion failed');
  }});
  try {
    script.runInNewContext({console}, {timeout:750});
    return {passed:checks>0, valid_syntax:true, error:checks>0?null:'NoAssertions'};
  } catch (error) { return {passed:false, valid_syntax:true, error:error.name}; }
});
process.stdout.write(JSON.stringify(results));
