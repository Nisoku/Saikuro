//  Frame codec (length-prefix, big-endian uint32)

export const MAX_FRAME_SIZE = 16 * 1024 * 1024; // 16 MiB

export function buildFrame(payload: Uint8Array): Buffer {
  const header = Buffer.allocUnsafe(4);
  header.writeUInt32BE(payload.length, 0);
  return Buffer.concat([header, payload]);
}
