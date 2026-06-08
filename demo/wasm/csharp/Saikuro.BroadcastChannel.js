const _channels = new Map();
const _messageQueues = {};
const _recvWaiters = {};

// Helper: push to queue and signal waiter
// Always pushes to the queue, then resolves any pending Promise<true>
// so C#'s WaitForRuntimeMessage awaits can proceed.  This mirrors the
// Rust adapter's mpsc channel where onmessage does try_send and the
// recv() future is woken by the channel.
function deliverMessage(connId, data) {
  const q = _messageQueues[connId];
  if (q) q.push(data);
  const waiter = _recvWaiters[connId];
  if (waiter) {
    _recvWaiters[connId] = null;
    waiter();
  }
}

globalThis.Saikuro_CreateBC = (name) => {
  const id = crypto.randomUUID();
  _channels.set(id, new BroadcastChannel(name));
  return id;
};

globalThis.Saikuro_PostMessage = (channelId, data) => {
  const ch = _channels.get(channelId);
  if (ch) ch.postMessage(data.buffer);
};

globalThis.Saikuro_CloseBC = (channelId) => {
  const ch = _channels.get(channelId);
  if (ch) { ch.close(); _channels.delete(channelId); }
};

// Runtime connection handshake

globalThis.Saikuro_ConnectToRuntime = (baseName) => {
  const connId = crypto.randomUUID();
  const privateName = baseName + ':' + connId;
  const pc = new BroadcastChannel(privateName);
  const internalId = '__rt_' + connId;
  _channels.set(internalId, pc);
  _messageQueues[connId] = [];

  // On incoming binary frames: deliver to waiter (if C# is waiting) or queue.
  pc.onmessage = (e2) => {
    if (e2.data instanceof ArrayBuffer) {
      deliverMessage(connId, new Uint8Array(e2.data));
    }
  };

  let accepted = false;

  return new Promise((resolve) => {
    const timer = setTimeout(() => {
      if (!accepted) {
        console.warn(
          `[Saikuro] runtime handshake timed out for ${connId}, ` +
          'proceeding without accept confirmation',
        );
      }
      resolve(connId);
    }, 5000);

    pc.addEventListener('message', function onAccept(e) {
      if (e.data && e.data.type === 'accept' && e.data.id === connId) {
        clearTimeout(timer);
        pc.removeEventListener('message', onAccept);
        accepted = true;
        resolve(connId);
      }
    });

    const base = new BroadcastChannel(baseName);
    base.postMessage({ type: 'connect', id: connId });
    base.close();
  });
};

// Push-based receive signal: like Rust's mpsc channel waker
// Returns true synchronously if data is already queued, or a Promise<true>
// that resolves when the next BroadcastChannel message arrives (via
// onmessage).  After it resolves, C# calls DequeueRuntimeMessage to
// get the actual bytes.  No polling / Task.Delay.
globalThis.Saikuro_WaitForRuntimeMessage = (connId) => {
  const q = _messageQueues[connId];
  if (q && q.length > 0) {
    return true;
  }
  return new Promise((resolve) => {
    _recvWaiters[connId] = () => resolve(true);
  });
};

// Legacy synchronous dequeue (called after WaitForRuntimeMessage signals).
globalThis.Saikuro_DequeueRuntimeMessage = (connId) => {
  const q = _messageQueues[connId];
  return (q && q.length > 0) ? q.shift() : null;
};

globalThis.Saikuro_SendRuntime = (connId, data) => {
  const ch = _channels.get('__rt_' + connId);
  if (ch) {
    // .NET [JSImport] marshals byte[] as ArrayBuffer, so data.buffer is
    // undefined on ArrayBuffer.  Post the ArrayBuffer directly.
    const buf = data instanceof Uint8Array ? data.buffer : data;
    ch.postMessage(buf);
  }
};

globalThis.Saikuro_CloseRuntime = (connId) => {
  const internalId = '__rt_' + connId;
  const ch = _channels.get(internalId);
  if (ch) { ch.close(); _channels.delete(internalId); }
  delete _recvWaiters[connId];
  delete _messageQueues[connId];
};
