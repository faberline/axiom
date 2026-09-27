import pytest

from candidate import Entry, FeedError, parse_feed

ATOM = b'<feed xmlns="http://www.w3.org/2005/Atom">%s</feed>'
FEED = b"""<?xml version="1.0"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Blog</title>
  <entry>
    <title>  First post </title>
    <link rel="edit" href="https://x/edit/1"/>
    <link rel="alternate" href="https://x/1"/>
    <updated>2026-01-01T00:00:00Z</updated>
  </entry>
  <entry>
    <title>Second</title>
    <link href="https://x/2"/>
  </entry>
</feed>
"""


def test_entries_are_extracted_with_namespaces():
    assert parse_feed(FEED) == [
        Entry("First post", "https://x/1", "2026-01-01T00:00:00Z"),
        Entry("Second", "https://x/2", ""),
    ]


def test_feed_without_entries_is_empty():
    assert parse_feed(b'<feed xmlns="http://www.w3.org/2005/Atom"/>') == []


def test_non_atom_root_is_rejected():
    with pytest.raises(FeedError, match="not an Atom feed"):
        parse_feed(b"<feed><entry><title>t</title></entry></feed>")
    with pytest.raises(FeedError, match="not an Atom feed"):
        parse_feed(b"<rss version='2.0'><channel/></rss>")


def test_entry_needs_title_and_link():
    doc = ATOM % b'<entry><title> </title><link href="u"/></entry>'
    with pytest.raises(FeedError, match="without title or link"):
        parse_feed(doc)
    doc = ATOM % b'<entry><title>t</title><link rel="edit" href="u"/></entry>'
    with pytest.raises(FeedError, match="without title or link"):
        parse_feed(doc)


def test_syntax_errors_are_wrapped():
    with pytest.raises(FeedError, match="invalid XML"):
        parse_feed(b"<feed")


def test_external_entities_are_not_resolved(tmp_path):
    secret = tmp_path / "secret.txt"
    secret.write_text("TOP-SECRET")
    doc = (
        f'<!DOCTYPE feed [<!ENTITY x SYSTEM "file://{secret}">]>'
        '<feed xmlns="http://www.w3.org/2005/Atom"><entry>'
        '<title>a &x; b</title><link href="u"/></entry></feed>'
    ).encode()
    entries = parse_feed(doc)
    assert "TOP-SECRET" not in entries[0].title
