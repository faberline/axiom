from datetime import date

import pytest
from lxml import etree

from candidate import MAX_URLS, SITEMAP_NS, Page, build_sitemap

NS = {"s": SITEMAP_NS}


def parse(data):
    return etree.fromstring(data)


def test_declaration_and_default_namespace():
    data = build_sitemap([Page("https://x.test/")])
    assert data.startswith(b"<?xml version='1.0' encoding='UTF-8'?>")
    root = parse(data)
    assert root.tag == f"{{{SITEMAP_NS}}}urlset"
    assert root.nsmap == {None: SITEMAP_NS}
    assert b"ns0:" not in data


def test_fields_are_written_in_order():
    root = parse(
        build_sitemap(
            [
                Page("https://x.test/a", date(2026, 3, 4), 0.8),
                Page("https://x.test/b"),
            ]
        )
    )
    urls = root.findall("s:url", NS)
    assert [u.findtext("s:loc", namespaces=NS) for u in urls] == [
        "https://x.test/a",
        "https://x.test/b",
    ]
    first = [etree.QName(child).localname for child in urls[0]]
    assert first == ["loc", "lastmod", "priority"]
    assert urls[0].findtext("s:lastmod", namespaces=NS) == "2026-03-04"
    assert urls[0].findtext("s:priority", namespaces=NS) == "0.8"
    assert len(urls[1]) == 1


def test_special_characters_are_escaped():
    loc = "https://x.test/search?q=a&b=<c>"
    data = build_sitemap([Page(loc)])
    assert b"q=a&amp;b=&lt;c&gt;" in data
    assert parse(data).findtext("s:url/s:loc", namespaces=NS) == loc


def test_non_ascii_is_utf8_encoded():
    data = build_sitemap([Page("https://x.test/café")])
    assert "café".encode() in data


def test_relative_loc_is_rejected():
    with pytest.raises(ValueError, match="absolute"):
        build_sitemap([Page("/a")])


def test_priority_bounds():
    build_sitemap(
        [Page("https://x.test/", priority=0.0), Page("https://x.test/1", priority=1.0)]
    )
    for bad in (-0.1, 1.5):
        with pytest.raises(ValueError, match="priority"):
            build_sitemap([Page("https://x.test/", priority=bad)])


def test_url_limit():
    pages = [Page("https://x.test/")] * MAX_URLS
    assert len(parse(build_sitemap(pages))) == MAX_URLS
    with pytest.raises(ValueError, match="at most"):
        build_sitemap(pages + [Page("https://x.test/")])


def test_empty_sitemap_is_valid():
    assert len(parse(build_sitemap([]))) == 0
