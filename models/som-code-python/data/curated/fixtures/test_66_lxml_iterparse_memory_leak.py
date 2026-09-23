"""Oracle test suite for 66-lxml-iterparse-memory-leak."""
import io
import pytest
from lxml import etree
import candidate


SAMPLE_STREAM_XML = b"""<?xml version="1.0" encoding="UTF-8"?>
<catalog>
  <item>
    <id>101</id>
    <name>Keyboard</name>
  </item>
  <item>
    <id>102</id>
    <name>Mouse</name>
  </item>
  <item>
    <id>103</id>
    <name>Monitor</name>
  </item>
</catalog>
"""


def test_streaming_record_extraction():
    extractor = candidate.StreamXmlRecordExtractor(target_tag="item")
    records = list(extractor.iter_records(io.BytesIO(SAMPLE_STREAM_XML)))
    assert len(records) == 3, "Must yield exactly 3 records matching the target tag"
    assert records[0] == {"id": "101", "name": "Keyboard"}
    assert records[1] == {"id": "102", "name": "Mouse"}
    assert records[2] == {"id": "103", "name": "Monitor"}


def test_element_clearing():
    extractor = candidate.StreamXmlRecordExtractor(target_tag="item")
    records = list(extractor.iter_records(io.BytesIO(SAMPLE_STREAM_XML)))
    assert len(records) == 3
    assert extractor.last_root is not None
    assert len(extractor.last_root) >= 1, "Root must contain at least the final element"
    last_item = extractor.last_root[0]
    assert len(last_item) == 0, "Element children must be cleared via elem.clear() to release DOM memory"


def test_parent_node_prunes_siblings():
    # Stream with 20 items
    items_xml = "".join(f"<item><id>{i}</id><val>test_{i}</val></item>" for i in range(20))
    full_xml = f"<root>{items_xml}</root>".encode("utf-8")

    extractor = candidate.StreamXmlRecordExtractor(target_tag="item")
    count = 0
    for rec in extractor.iter_records(io.BytesIO(full_xml)):
        count += 1
        assert "id" in rec and "val" in rec

    assert count == 20
    assert extractor.last_root is not None
    assert len(extractor.last_root) <= 1, (
        f"Parent root accumulated {len(extractor.last_root)} child elements;"
        " must prune siblings with del parent[0]"
    )


def test_uses_end_events_configuration(monkeypatch):
    called_events = []
    orig_iterparse = etree.iterparse

    def tracking_iterparse(source, *args, **kwargs):
        called_events.append(kwargs.get("events"))
        return orig_iterparse(source, *args, **kwargs)

    monkeypatch.setattr(etree, "iterparse", tracking_iterparse)
    extractor = candidate.StreamXmlRecordExtractor(target_tag="item")
    list(extractor.iter_records(io.BytesIO(SAMPLE_STREAM_XML)))
    assert len(called_events) == 1
    assert called_events[0] == ("end",), (
        "iterparse must use events=('end',) to process completed elements"
    )


def test_empty_tag_validation():
    with pytest.raises(ValueError, match="target_tag"):
        candidate.StreamXmlRecordExtractor(target_tag="")
    with pytest.raises(ValueError, match="target_tag"):
        candidate.StreamXmlRecordExtractor(target_tag="   ")
