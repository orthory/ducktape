import assert from 'node:assert/strict';
import { Evidence } from './proxmox-view-observe.mjs';
const root = 'ab'.repeat(32);
const frame = (height, root_hash = root) => ({ type: 'heartbeat', height, root_hash });
const evidence = new Evidence();
assert.equal(evidence.accept(0, frame(0, "")), null, "unprimed native hub is not a committed head");
for (let node = 0; node < 3; node++) assert.equal(evidence.accept(node, frame(5)), null);
for (let repeat = 0; repeat < 20; repeat++) {
  for (let node = 0; node < 3; node++) assert.equal(evidence.accept(node, frame(5)), null,
    'unchanged heartbeat must not prove consensus progress');
}
assert.equal(evidence.accept(0, frame(6)), null);
assert.equal(evidence.accept(1, frame(6)), null);
assert.deepEqual(evidence.accept(2, frame(6)), { height: 6, root_hash: root });
const divergent = new Evidence();
for (let node = 0; node < 3; node++) divergent.accept(node, frame(5));
divergent.accept(0, frame(6));
assert.throws(() => divergent.accept(1, frame(6, 'cd'.repeat(32))), /different roots/);
assert.throws(() => evidence.accept(0, frame(4)), /height regressed/);
assert.throws(() => evidence.accept(0, frame(7, 'not-a-root')), /invalid heartbeat/);
console.log('heartbeat progress, root agreement, divergence and regression checks passed');
