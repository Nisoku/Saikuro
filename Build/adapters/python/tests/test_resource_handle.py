"""
Tests for decodeResourceHandle
"""

import pytest
from saikuro.envelope import ResourceHandle


class TestResourceHandleFromDict:
    def test_decodes_minimal_handle(self):
        raw = {"id": "res-1"}
        result = ResourceHandle.from_dict(raw)
        assert result == ResourceHandle(id="res-1")

    def test_decodes_fully_populated_handle(self):
        raw = {
            "id": "res-2",
            "mime_type": "image/png",
            "size": 4096,
            "uri": "saikuro://res/res-2",
        }
        result = ResourceHandle.from_dict(raw)
        assert result == ResourceHandle(
            id="res-2", mime_type="image/png", size=4096, uri="saikuro://res/res-2"
        )

    def test_raises_for_null(self):
        with pytest.raises(ValueError, match="expected dict"):
            ResourceHandle.from_dict(None)

    def test_raises_for_missing_id(self):
        raw = {"mime_type": "text/plain"}
        with pytest.raises(ValueError, match="missing or non-string"):
            ResourceHandle.from_dict(raw)

    def test_raises_when_id_not_string(self):
        raw = {"id": 42}
        with pytest.raises(ValueError, match="missing or non-string"):
            ResourceHandle.from_dict(raw)

    def test_omits_absent_optional_fields(self):
        raw = {"id": "x"}
        result = ResourceHandle.from_dict(raw)
        assert result.id == "x"
        assert result.mime_type is None
        assert result.size is None
        assert result.uri is None

    def test_drops_unknown_extra_fields(self):
        raw = {"id": "x", "unknown_field": True, "another": 123}
        result = ResourceHandle.from_dict(raw)
        assert not hasattr(result, "unknown_field")
        assert not hasattr(result, "another")
