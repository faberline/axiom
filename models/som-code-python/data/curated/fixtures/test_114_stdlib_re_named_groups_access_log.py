from datetime import datetime, timedelta, timezone

import pytest

from candidate import parse_line, scan, server_errors

GOOD = '10.0.0.7 [10/Oct/2026:13:55:36 +0200] "GET /api/items" 200 512'


def test_fields_are_extracted_and_typed():
    req = parse_line(GOOD)
    assert (req.ip, req.method, req.path, req.status, req.size) == ("10.0.0.7", "GET", "/api/items", 200, 512)
    assert req.ts == datetime(2026, 10, 10, 13, 55, 36, tzinfo=timezone(timedelta(hours=2)))


def test_dash_size_means_zero_bytes():
    assert parse_line('10.0.0.7 [10/Oct/2026:13:55:36 +0000] "HEAD /" 304 -').size == 0


def test_trailing_garbage_and_unknown_methods_are_malformed():
    with pytest.raises(ValueError, match="malformed log line"):
        parse_line(GOOD + " extra")
    with pytest.raises(ValueError, match="malformed log line"):
        parse_line('10.0.0.7 [10/Oct/2026:13:55:36 +0000] "BREW /pot" 418 0')


def test_scan_skips_blank_lines_and_counts_malformed_ones():
    requests, malformed = scan(["", GOOD, "garbage", "   "])
    assert [r.path for r in requests] == ["/api/items"]
    assert malformed == 1


def test_server_errors_include_500():
    lines = [GOOD.replace(" 200 ", f" {code} ") for code in (499, 500, 503)]
    requests, _ = scan(lines)
    assert server_errors(requests) == ["/api/items", "/api/items"]
