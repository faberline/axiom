import pytest

from candidate import OrderValidator, SchemaLoadError, Violation

XSD = b"""<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
  <xs:element name="order">
    <xs:complexType>
      <xs:sequence>
        <xs:element name="item" maxOccurs="unbounded">
          <xs:complexType>
            <xs:attribute name="sku" type="xs:string" use="required"/>
            <xs:attribute name="qty" type="xs:positiveInteger" use="required"/>
          </xs:complexType>
        </xs:element>
      </xs:sequence>
    </xs:complexType>
  </xs:element>
</xs:schema>
"""

GOOD = b'<order>\n<item sku="a" qty="1"/>\n<item sku="b" qty="3"/>\n</order>'
BAD = (
    b'<order>\n<item sku="a" qty="1"/>\n<item qty="0"/>\n'
    b'<item sku="c" qty="x"/>\n</order>'
)


@pytest.fixture
def validator():
    return OrderValidator(XSD)


def test_valid_document_has_no_violations(validator):
    assert validator.violations(GOOD) == []
    assert validator.is_valid(GOOD)


def test_every_violation_is_reported_with_its_line(validator):
    found = validator.violations(BAD)
    assert [v.line for v in found] == sorted(v.line for v in found)
    assert {v.line for v in found} == {3, 4}
    assert len(found) >= 3
    assert any("sku" in v.message for v in found if v.line == 3)
    assert not validator.is_valid(BAD)


def test_errors_do_not_leak_between_documents(validator):
    validator.violations(BAD)
    assert validator.violations(GOOD) == []
    only = b'<order>\n\n\n\n<item sku="z" qty="-1"/>\n</order>'
    assert {v.line for v in validator.violations(only)} == {5}


def test_syntax_errors_become_a_violation(validator):
    found = validator.violations(b"<order>\n<item sku='a' qty='1'>\n</order>")
    assert len(found) == 1
    assert found[0].message.startswith("syntax:")
    assert found[0].line == 3


def test_malformed_schema_raises():
    with pytest.raises(SchemaLoadError):
        OrderValidator(b"<xs:schema")
    with pytest.raises(SchemaLoadError):
        OrderValidator(XSD.replace(b"xs:sequence>", b"xs:bogus>"))


def test_violation_is_a_value():
    assert Violation(1, "m") == Violation(1, "m")
