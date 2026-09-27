"""Parse access-log lines with a compiled regular expression and named groups."""

import re
from collections.abc import Iterable
from dataclasses import dataclass
from datetime import datetime

LINE = re.compile(
    r"(?P<ip>\d{1,3}(?:\.\d{1,3}){3}) "
    r"\[(?P<ts>[^\]]+)\] "
    r'"(?P<method>\w+) (?P<path>\S+)" '
    r"(?P<status>\d{3}) (?P<size>\d+|-)"
)
TIMESTAMP = "%d/%b/%Y:%H:%M:%S %z"


@dataclass(frozen=True, slots=True)
class Request:
    """One parsed access-log entry."""

    ip: str
    ts: datetime
    method: str
    path: str
    status: int
    size: int


def parse_line(line: str) -> Request:
    """Parse one whole line, rejecting anything the pattern does not fully match."""
    match = LINE.fullmatch(line.strip())
    if match is None:
        raise ValueError(f"malformed log line: {line!r}")
    size = 0 if match["size"] == "-" else int(match["size"])
    return Request(
        ip=match["ip"],
        ts=datetime.strptime(match["ts"], TIMESTAMP),
        method=match["method"],
        path=match["path"],
        status=int(match["status"]),
        size=size,
    )


def scan(lines: Iterable[str]) -> tuple[list[Request], int]:
    """Parse every non-blank line, returning the requests and the malformed count."""
    requests: list[Request] = []
    malformed = 0
    for line in lines:
        if not line.strip():
            continue
        try:
            requests.append(parse_line(line))
        except ValueError:
            malformed += 1
    return requests, malformed


def server_errors(requests: Iterable[Request]) -> list[str]:
    """Return the paths of requests that ended in a 5xx status."""
    return [request.path for request in requests if request.status >= 500]
