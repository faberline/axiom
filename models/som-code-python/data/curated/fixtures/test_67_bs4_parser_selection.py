"""Oracle test suite for 67-bs4-parser-selection."""
import pytest
from bs4 import BeautifulSoup
import candidate


XML_WITH_CDATA = """<?xml version="1.0" encoding="utf-8"?>
<dataStore>
  <ConfigEntry>
    <Payload><![CDATA[<raw_json>{"key": "value & more"}</raw_json>]]></Payload>
  </ConfigEntry>
</dataStore>
"""

XML_WITH_CASE_SENSITIVE_TAGS = """<?xml version="1.0" encoding="utf-8"?>
<Registry>
  <DeviceID>DEV-9901</DeviceID>
  <deviceid>lowercase-tag</deviceid>
</Registry>
"""

XML_SELF_CLOSING = """<catalog><item id="1"/><item id="2"/></catalog>"""


def test_xml_cdata_extraction():
    parser = candidate.StructuredMarkupParser(mode="xml")
    values = parser.extract_tag_values(XML_WITH_CDATA, "Payload")
    assert len(values) == 1
    assert values[0].startswith("<raw_json>"), "CDATA extraction must preserve opening markup boundary"
    assert '<raw_json>{"key": "value & more"}</raw_json>' in values[0]


def test_xml_case_sensitivity():
    parser = candidate.StructuredMarkupParser(mode="xml")
    upper = parser.extract_tag_values(XML_WITH_CASE_SENSITIVE_TAGS, "DeviceID")
    assert upper == ["DEV-9901"]

    lower = parser.extract_tag_values(XML_WITH_CASE_SENSITIVE_TAGS, "deviceid")
    assert lower == ["lowercase-tag"]


def test_xml_self_closing_tags_structure():
    parser = candidate.StructuredMarkupParser(mode="xml")
    soup = parser.parse(XML_SELF_CLOSING)
    items = soup.find_all("item")
    assert len(items) == 2
    assert items[0].parent == items[1].parent, "XML parser must maintain sibling structure for self-closing tags"


def test_explicit_parser_feature():
    p_xml = candidate.StructuredMarkupParser(mode="xml")
    assert p_xml.parser_feature == "lxml-xml"
    soup = p_xml.parse("<root><val>1</val></root>")
    assert soup.builder.is_xml is True

    p_html = candidate.StructuredMarkupParser(mode="html")
    assert p_html.parser_feature == "lxml"


def test_validation_errors():
    with pytest.raises(ValueError, match="mode"):
        candidate.StructuredMarkupParser(mode="unsupported_mode")
    parser = candidate.StructuredMarkupParser()
    with pytest.raises(ValueError, match="markup"):
        parser.parse("")
    with pytest.raises(ValueError, match="tag_name"):
        parser.extract_tag_values("<root/>", "")
