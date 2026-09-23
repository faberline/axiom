"""Oracle test suite for 65-lxml-xxe-injection."""
import pytest
from lxml import etree
import candidate


XXE_PAYLOAD = """<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE test [
  <!ENTITY secret "UNAUTHORIZED_ENTITY_EXPANSION">
]>
<user>
  <name>Alice</name>
  <data>&secret;</data>
</user>
"""

BENIGN_XML = """<?xml version="1.0" encoding="UTF-8"?>
<user>
  <name>Bob</name>
  <role>Admin</role>
</user>
"""


def test_xxe_entity_expansion_blocked():
    parser = candidate.SecureXmlParser()
    data = parser.extract_flat_dict(XXE_PAYLOAD)
    assert data["name"] == "Alice"
    assert data["data"] != "UNAUTHORIZED_ENTITY_EXPANSION", "XXE entity expansion must not occur"


def test_parser_configuration_flags():
    parser = candidate.SecureXmlParser()
    assert parser.resolve_entities is False
    assert parser.no_network is True
    assert parser.load_dtd is False


def test_uses_hardened_parser_instance(monkeypatch):
    parser = candidate.SecureXmlParser()
    called_parsers = []
    orig_fromstring = etree.fromstring

    def tracking_fromstring(data, *args, **kwargs):
        called_parsers.append(kwargs.get("parser"))
        return orig_fromstring(data, *args, **kwargs)

    monkeypatch.setattr(etree, "fromstring", tracking_fromstring)
    parser.parse_string("<root><item>test</item></root>")
    assert len(called_parsers) == 1
    assert called_parsers[0] is parser._parser, "parse_string must pass self._parser to etree.fromstring"


def test_reject_unsafe_resolve_entities():
    with pytest.raises(ValueError, match="resolve_entities"):
        candidate.SecureXmlParser(resolve_entities=True)


def test_reject_unsafe_no_network():
    with pytest.raises(ValueError, match="no_network"):
        candidate.SecureXmlParser(no_network=False)


def test_benign_xml_parsing():
    parser = candidate.SecureXmlParser()
    data = parser.extract_flat_dict(BENIGN_XML)
    assert data == {"name": "Bob", "role": "Admin"}


def test_empty_input_validation():
    parser = candidate.SecureXmlParser()
    with pytest.raises(ValueError, match="empty"):
        parser.parse_string("")
    with pytest.raises(ValueError, match="empty"):
        parser.parse_string("   ")
