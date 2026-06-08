export type MessageRecord = {
  id: string;
  stage: string;
  language: string;
  direction: "call" | "response";
  kind: string;
  serialized: string;
  timestamp: number;
};

export class MessageLog {
  private records: MessageRecord[] = [];

  add(record: MessageRecord): void {
    this.records.unshift(record);
  }

  list(): MessageRecord[] {
    return [...this.records];
  }

  clear(): void {
    this.records = [];
  }
}
