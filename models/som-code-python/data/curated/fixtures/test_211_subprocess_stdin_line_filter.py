import subprocess
import sys

import pytest

from candidate import run_filter

UPPER = "import sys\nfor line in sys.stdin:\n    sys.stdout.write(line.upper())"
COUNT = "import sys; print(sys.stdin.read().count(chr(10)))"
BAD_BYTES = "import sys; sys.stdout.buffer.write(b'ok \\xff\\n')"


def test_lines_round_trip_through_the_filter():
    assert run_filter([sys.executable, "-c", UPPER], ["héllo", "wörld"]) == [
        "HÉLLO",
        "WÖRLD",
    ]


def test_every_line_is_newline_terminated():
    assert run_filter([sys.executable, "-c", COUNT], ["a", "b", "c"]) == ["3"]
    assert run_filter([sys.executable, "-c", COUNT], []) == ["0"]


def test_no_trailing_empty_line():
    assert run_filter([sys.executable, "-c", UPPER], ["x"]) == ["X"]
    assert run_filter([sys.executable, "-c", UPPER], []) == []


def test_undecodable_output_is_replaced():
    assert run_filter([sys.executable, "-c", BAD_BYTES], []) == ["ok �"]


def test_bad_input_and_failures_raise():
    with pytest.raises(ValueError, match="line break"):
        run_filter([sys.executable, "-c", UPPER], ["ok", "two\nlines"])
    with pytest.raises(subprocess.CalledProcessError):
        run_filter([sys.executable, "-c", "raise SystemExit(2)"], ["a"])
