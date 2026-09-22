"""Secure XML parser preventing XML External Entity (XXE) vulnerabilities."""
from typing import Any, Dict, Union
from lxml import etree


class SecureXmlParser:
    """Wraps lxml.etree to safely parse untrusted XML with XXE protections."""

    def __init__(
        self,
        resolve_entities: bool = True,
        no_network: bool = True,
        load_dtd: bool = False,
    ) -> None:
        if not resolve_entities:
            raise ValueError("resolve_entities must be False to prevent XXE attacks")
        if not no_network:
            raise ValueError("no_network must be True to prevent network entity expansion")

        self.resolve_entities = resolve_entities
        self.no_network = no_network
        self.load_dtd = load_dtd
        self._parser = etree.XMLParser(
            resolve_entities=resolve_entities,
            no_network=no_network,
            load_dtd=load_dtd,
        )

    def parse_string(self, xml_content: Union[str, bytes]) -> etree._Element:
        """Parse XML string or bytes safely using the hardened parser."""
        if not xml_content or (isinstance(xml_content, str) and not xml_content.strip()):
            raise ValueError("xml_content cannot be empty")
        data = xml_content.encode("utf-8") if isinstance(xml_content, str) else xml_content
        return etree.fromstring(data, parser=self._parser)

    def extract_flat_dict(self, xml_content: Union[str, bytes]) -> Dict[str, str]:
        """Parse XML and extract immediate child tag text as a dictionary."""
        root = self.parse_string(xml_content)
        return {child.tag: (child.text or "").strip() for child in root}
