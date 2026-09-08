// Native Node >=22 WebSocket: no package installation or protocol implementation.
import { pathToFileURL } from 'node:url';

export class Evidence {
  nodes = Array.from({ length: 3 }, () => ({ initial: null, latest: null, roots: new Map() }));

  accept(index, frame) {
    if (frame.type !== 'heartbeat') return null;
    const { height, root_hash: root } = frame;
    // The native hub sends (0, "") before its first committed tip.
    if (height === 0 && root === "" && this.nodes[index].latest === null) return null;
    if (!Number.isSafeInteger(height) || height < 0 || !/^[a-f0-9]{64}$/.test(root)) {
      throw new Error('invalid heartbeat');
    }
    const node = this.nodes[index];
    if (node.latest !== null && height < node.latest) throw new Error('height regressed');
    node.initial ??= height;
    node.latest = height;
    for (const other of this.nodes) {
      const old = other.roots.get(height);
      if (old !== undefined && old !== root) throw new Error(`different roots at height ${height}`);
    }
    node.roots.set(height, root);
    if (node.roots.size > 64) node.roots.delete(node.roots.keys().next().value);
    if (this.nodes.some(n => n.initial === null)) return null;
    const baseline = Math.max(...this.nodes.map(n => n.initial));
    if (height > baseline && this.nodes.every(n => n.roots.get(height) === root)) {
      return { height, root_hash: root };
    }
    return null;
  }
}

async function observe(urls) {
  if (urls.length !== 3 || new Set(urls).size !== 3) throw new Error('provide three distinct forwarded ws:// URLs');
  for (const input of urls) {
    const url = new URL(input);
    if (url.protocol !== 'ws:' || !['127.0.0.1', 'localhost', '[::1]'].includes(url.hostname)
        || url.pathname !== '/v1/ws' || url.search || url.username || url.password) {
      throw new Error('use loopback SSH forwards to /v1/ws');
    }
  }
  const sockets = [];
  let timer;
  try {
    const result = await new Promise((resolve, reject) => {
      const evidence = new Evidence();
      timer = setTimeout(() => reject(new Error('no advancing common committed root before deadline')), 180_000);
      urls.forEach((url, index) => {
        const socket = new WebSocket(url);
        sockets.push(socket);
        socket.addEventListener('message', ({ data }) => {
          try {
            const result = evidence.accept(index, JSON.parse(data));
            if (result) resolve(result);
          } catch (error) { reject(error); }
        });
        socket.addEventListener('error', () => reject(new Error(`node ${index} websocket failed`)));
        socket.addEventListener('close', () => reject(new Error(`node ${index} websocket closed`)));
      });
    });
    console.log(JSON.stringify(result));
  } finally {
    clearTimeout(timer);
    for (const socket of sockets) socket.close();
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  observe(process.argv.slice(2)).catch(error => { console.error(error.message); process.exitCode = 1; });
}
