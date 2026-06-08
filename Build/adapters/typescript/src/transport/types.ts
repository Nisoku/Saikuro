export interface Transport {
  /** Connect (or initialise) the transport. */
  connect(): Promise<void>;

  /** Close the transport gracefully. */
  close(): Promise<void>;

  /** Serialise `obj` to MessagePack and transmit it. */
  send(obj: object): Promise<void>;

  /**
   * Receive and deserialise the next message.
   * Returns `null` when the connection has been closed by the peer.
   */
  recv(): Promise<Record<string, unknown> | null>;

  /** Register a handler invoked for every received message. */
  onMessage(handler: (msg: Record<string, unknown>) => void): void;

  /** Deregister a previously-registered message handler. */
  offMessage(handler: (msg: Record<string, unknown>) => void): void;

  /** Register an error / close handler. */
  onClose(handler: (err?: Error) => void): void;
}
