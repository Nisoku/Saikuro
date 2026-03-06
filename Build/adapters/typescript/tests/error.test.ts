/**
 * Tests for the SaikuroError class hierarchy
 */

import { describe, it, expect } from "vitest";
import {
  SaikuroError,
  FunctionNotFoundError,
  InvalidArgumentsError,
  CapabilityDeniedError,
  TransportError,
  SaikuroTimeoutError,
  NoProviderError,
  ProviderUnavailableError,
  ProviderError,
  MalformedEnvelopeError,
  MessageTooLargeError,
  BufferOverflowError,
  StreamClosedError,
  ChannelClosedError,
  OutOfOrderError,
} from "../src/error";

const p = (
  code: ConstructorParameters<typeof SaikuroError>[0]["code"],
  message = "msg",
) => ({ code, message }) as const;

describe("SaikuroError.fromPayload :  subclass mapping", () => {
  it("FunctionNotFound → FunctionNotFoundError", () => {
    expect(SaikuroError.fromPayload(p("FunctionNotFound"))).toBeInstanceOf(
      FunctionNotFoundError,
    );
  });

  it("NamespaceNotFound → FunctionNotFoundError", () => {
    expect(SaikuroError.fromPayload(p("NamespaceNotFound"))).toBeInstanceOf(
      FunctionNotFoundError,
    );
  });

  it("InvalidArguments → InvalidArgumentsError", () => {
    expect(SaikuroError.fromPayload(p("InvalidArguments"))).toBeInstanceOf(
      InvalidArgumentsError,
    );
  });

  it("CapabilityDenied → CapabilityDeniedError", () => {
    expect(SaikuroError.fromPayload(p("CapabilityDenied"))).toBeInstanceOf(
      CapabilityDeniedError,
    );
  });

  it("ConnectionLost → TransportError", () => {
    expect(SaikuroError.fromPayload(p("ConnectionLost"))).toBeInstanceOf(
      TransportError,
    );
  });

  it("Timeout → SaikuroTimeoutError", () => {
    expect(SaikuroError.fromPayload(p("Timeout"))).toBeInstanceOf(
      SaikuroTimeoutError,
    );
  });

  it("NoProvider → NoProviderError", () => {
    expect(SaikuroError.fromPayload(p("NoProvider"))).toBeInstanceOf(
      NoProviderError,
    );
  });

  it("ProviderUnavailable → ProviderUnavailableError", () => {
    expect(SaikuroError.fromPayload(p("ProviderUnavailable"))).toBeInstanceOf(
      ProviderUnavailableError,
    );
  });

  it("ProviderError → ProviderError", () => {
    expect(SaikuroError.fromPayload(p("ProviderError"))).toBeInstanceOf(
      ProviderError,
    );
  });

  it("MalformedEnvelope → MalformedEnvelopeError", () => {
    expect(SaikuroError.fromPayload(p("MalformedEnvelope"))).toBeInstanceOf(
      MalformedEnvelopeError,
    );
  });

  it("MessageTooLarge → MessageTooLargeError", () => {
    expect(SaikuroError.fromPayload(p("MessageTooLarge"))).toBeInstanceOf(
      MessageTooLargeError,
    );
  });

  it("BufferOverflow → BufferOverflowError", () => {
    expect(SaikuroError.fromPayload(p("BufferOverflow"))).toBeInstanceOf(
      BufferOverflowError,
    );
  });

  it("StreamClosed → StreamClosedError", () => {
    expect(SaikuroError.fromPayload(p("StreamClosed"))).toBeInstanceOf(
      StreamClosedError,
    );
  });

  it("ChannelClosed → ChannelClosedError", () => {
    expect(SaikuroError.fromPayload(p("ChannelClosed"))).toBeInstanceOf(
      ChannelClosedError,
    );
  });

  it("OutOfOrder → OutOfOrderError", () => {
    expect(SaikuroError.fromPayload(p("OutOfOrder"))).toBeInstanceOf(
      OutOfOrderError,
    );
  });

  it("Internal (unmapped) → base SaikuroError", () => {
    expect(SaikuroError.fromPayload(p("Internal"))).toBeInstanceOf(
      SaikuroError,
    );
  });
});

describe("SaikuroError fields", () => {
  it("exposes .code", () => {
    const e = SaikuroError.fromPayload(p("Timeout"));
    expect(e.code).toBe("Timeout");
  });

  it("includes code and message in Error.message", () => {
    const e = SaikuroError.fromPayload({
      code: "Internal",
      message: "something went wrong",
    });
    expect(e.message).toContain("Internal");
    expect(e.message).toContain("something went wrong");
  });

  it("exposes .details when provided", () => {
    const e = SaikuroError.fromPayload({
      code: "Internal",
      message: "err",
      details: { hint: "check logs" },
    });
    expect(e.details["hint"]).toBe("check logs");
  });

  it(".details defaults to empty object when absent", () => {
    const e = SaikuroError.fromPayload({ code: "Internal", message: "err" });
    expect(e.details).toEqual({});
  });

  it("all subclasses are instanceof SaikuroError", () => {
    const subclasses = [
      FunctionNotFoundError,
      InvalidArgumentsError,
      CapabilityDeniedError,
      TransportError,
      SaikuroTimeoutError,
      NoProviderError,
      ProviderUnavailableError,
      ProviderError,
      MalformedEnvelopeError,
      MessageTooLargeError,
      BufferOverflowError,
      StreamClosedError,
      ChannelClosedError,
      OutOfOrderError,
    ];
    for (const Cls of subclasses) {
      expect(new Cls(p("Internal"))).toBeInstanceOf(SaikuroError);
    }
  });
});
