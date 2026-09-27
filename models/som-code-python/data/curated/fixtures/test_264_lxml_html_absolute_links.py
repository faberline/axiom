import pytest

from candidate import PageError, extract_links

BASE = "https://example.com/docs/guide/"


def test_relative_links_are_made_absolute():
    page = '<a href="intro.html">i</a><a href="../api/">a</a><a href="/about">b</a>'
    assert extract_links(page, BASE) == [
        "https://example.com/docs/guide/intro.html",
        "https://example.com/docs/api/",
        "https://example.com/about",
    ]


def test_base_href_overrides_the_page_url():
    page = (
        '<html><head><base href="https://cdn.example.org/v2/"></head>'
        '<body><a href="x.html">x</a></body></html>'
    )
    assert extract_links(page, BASE) == ["https://cdn.example.org/v2/x.html"]


def test_fragments_are_dropped_and_duplicates_removed():
    page = (
        '<a href="a#top">1</a><a href="a#end">2</a><a href="b">3</a><a href="a">4</a>'
    )
    assert extract_links(page, BASE) == [
        "https://example.com/docs/guide/a",
        "https://example.com/docs/guide/b",
    ]


def test_non_http_schemes_are_skipped():
    page = (
        '<a href="mailto:x@y.z">m</a><a href="javascript:void(0)">j</a>'
        '<a href="ftp://host/f">f</a><a href="tel:123">t</a>'
        '<a href="http://other.test/">ok</a>'
    )
    assert extract_links(page, BASE) == ["http://other.test/"]


def test_nofollow_and_missing_href_are_skipped():
    page = (
        '<a>none</a><a name="top">anchor</a>'
        '<a rel="noopener NoFollow" href="/ads">ad</a><a rel="noopener" href="/k">k</a>'
    )
    assert extract_links(page, BASE) == ["https://example.com/k"]


def test_empty_document_raises():
    with pytest.raises(PageError, match="empty document"):
        extract_links("   \n", BASE)
