"""
Tests for the SaikuroError class hierarchy
"""

from saikuro.error import (
    SaikuroError,
    FunctionNotFoundError,
    InvalidArgumentsError,
    CapabilityDeniedError,
    TransportError,
    TimeoutError,
    NoProviderError,
    ProviderUnavailableError,
    ProviderError,
    MalformedEnvelopeError,
    MessageTooLargeError,
    BufferOverflowError,
    StreamClosedError,
    ChannelClosedError,
    OutOfOrderError,
)


def _p(code: str, message: str = "msg") -> dict:
    return {"code": code, "message": message}


class TestFromErrorDictSubclassMapping:
    def test_function_not_found(self):
        assert isinstance(
            SaikuroError.from_error_dict(_p("FunctionNotFound")), FunctionNotFoundError
        )

    def test_namespace_not_found(self):
        assert isinstance(
            SaikuroError.from_error_dict(_p("NamespaceNotFound")), FunctionNotFoundError
        )

    def test_invalid_arguments(self):
        assert isinstance(
            SaikuroError.from_error_dict(_p("InvalidArguments")), InvalidArgumentsError
        )

    def test_capability_denied(self):
        assert isinstance(
            SaikuroError.from_error_dict(_p("CapabilityDenied")), CapabilityDeniedError
        )

    def test_capability_invalid(self):
        assert isinstance(
            SaikuroError.from_error_dict(_p("CapabilityInvalid")), CapabilityDeniedError
        )

    def test_connection_lost(self):
        assert isinstance(
            SaikuroError.from_error_dict(_p("ConnectionLost")), TransportError
        )

    def test_timeout(self):
        assert isinstance(SaikuroError.from_error_dict(_p("Timeout")), TimeoutError)

    def test_no_provider(self):
        assert isinstance(
            SaikuroError.from_error_dict(_p("NoProvider")), NoProviderError
        )

    def test_provider_unavailable(self):
        assert isinstance(
            SaikuroError.from_error_dict(_p("ProviderUnavailable")),
            ProviderUnavailableError,
        )

    def test_provider_error(self):
        assert isinstance(
            SaikuroError.from_error_dict(_p("ProviderError")), ProviderError
        )

    def test_provider_panic(self):
        assert isinstance(
            SaikuroError.from_error_dict(_p("ProviderPanic")), ProviderError
        )

    def test_malformed_envelope(self):
        assert isinstance(
            SaikuroError.from_error_dict(_p("MalformedEnvelope")),
            MalformedEnvelopeError,
        )

    def test_message_too_large(self):
        assert isinstance(
            SaikuroError.from_error_dict(_p("MessageTooLarge")), MessageTooLargeError
        )

    def test_buffer_overflow(self):
        assert isinstance(
            SaikuroError.from_error_dict(_p("BufferOverflow")), BufferOverflowError
        )

    def test_stream_closed(self):
        assert isinstance(
            SaikuroError.from_error_dict(_p("StreamClosed")), StreamClosedError
        )

    def test_channel_closed(self):
        assert isinstance(
            SaikuroError.from_error_dict(_p("ChannelClosed")), ChannelClosedError
        )

    def test_out_of_order(self):
        assert isinstance(
            SaikuroError.from_error_dict(_p("OutOfOrder")), OutOfOrderError
        )

    def test_unmapped_code_falls_back_to_base(self):
        err = SaikuroError.from_error_dict(_p("Internal"))
        assert type(err) is SaikuroError


class TestSaikuroErrorFields:
    def test_code_accessible(self):
        err = SaikuroError.from_error_dict(_p("Timeout"))
        assert err.code == "Timeout"

    def test_message_accessible(self):
        err = SaikuroError.from_error_dict(
            {"code": "Internal", "message": "something went wrong"}
        )
        assert err.message == "something went wrong"

    def test_str_contains_code_and_message(self):
        err = SaikuroError.from_error_dict({"code": "Internal", "message": "oops"})
        s = str(err)
        assert "Internal" in s
        assert "oops" in s

    def test_details_populated(self):
        err = SaikuroError.from_error_dict(
            {
                "code": "Internal",
                "message": "err",
                "details": {"hint": "check logs"},
            }
        )
        assert err.details["hint"] == "check logs"

    def test_details_defaults_to_empty_dict(self):
        err = SaikuroError.from_error_dict({"code": "Internal", "message": "err"})
        assert err.details == {}

    def test_all_subclasses_are_saikuro_error(self):
        subclasses = [
            FunctionNotFoundError,
            InvalidArgumentsError,
            CapabilityDeniedError,
            TransportError,
            TimeoutError,
            NoProviderError,
            ProviderUnavailableError,
            ProviderError,
            MalformedEnvelopeError,
            MessageTooLargeError,
            BufferOverflowError,
            StreamClosedError,
            ChannelClosedError,
            OutOfOrderError,
        ]
        for klass in subclasses:
            assert issubclass(klass, SaikuroError)
            instance = klass(code="Internal", message="test")
            assert isinstance(instance, SaikuroError)
