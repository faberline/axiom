import io
import pytest
from candidate import StreamSafeDataProcessor


class TrackingStringIO(io.StringIO):
    def __init__(self, initial_value=""):
        super().__init__(initial_value)
        self.lines_read = 0

    def __iter__(self):
        return self

    def __next__(self):
        line = super().readline()
        if not line:
            raise StopIteration
        self.lines_read += 1
        return line

    def readline(self, size=-1):
        line = super().readline(size)
        if line:
            self.lines_read += 1
        return line


def test_deep_json_recursion_rejected():
    proc = StreamSafeDataProcessor()
    nested = '{"a": ' * 30 + '1' + '}' * 30
    with pytest.raises(ValueError):
        proc.safe_parse_json(nested, max_depth=20)


def test_safe_parse_json_valid():
    proc = StreamSafeDataProcessor()
    valid_json = '{"user": {"name": "Alice", "tags": ["admin", "dev"]}}'
    res = proc.safe_parse_json(valid_json, max_depth=20)
    assert res["user"]["name"] == "Alice"


def test_safe_parse_json_default_depth():
    proc = StreamSafeDataProcessor()
    nested_25 = '{"x": ' * 25 + '42' + '}' * 25
    with pytest.raises(ValueError):
        proc.safe_parse_json(nested_25)


def test_stream_csv_records_generator_chunking():
    proc = StreamSafeDataProcessor()
    csv_data = "id,name\n1,a\n2,b\n3,c\n4,d\n5,e\n"
    f = io.StringIO(csv_data)
    batches = list(proc.stream_csv_records(f, batch_size=2))
    assert len(batches) == 3
    assert len(batches[0]) == 2
    assert len(batches[1]) == 2
    assert len(batches[2]) == 1
    assert batches[2][0]["id"] == "5"


def test_stream_csv_records_does_not_eagerly_read_entire_file():
    proc = StreamSafeDataProcessor()
    csv_data = "id,val\n" + "\n".join(f"{i},{i*10}" for i in range(1, 15)) + "\n"
    f = TrackingStringIO(csv_data)
    gen = proc.stream_csv_records(f, batch_size=2)
    first_batch = next(gen)
    assert len(first_batch) == 2
    assert first_batch[0]["id"] == "1"
    assert f.lines_read <= 4, f"Eager reading detected: read {f.lines_read} lines for batch of 2"


def test_compute_column_summary_empty_csv():
    proc = StreamSafeDataProcessor()
    empty_csv = "id,score\n"
    f = io.StringIO(empty_csv)
    summary = proc.compute_column_summary(f, "score")
    assert summary["count"] == 0.0
    assert summary["mean"] == 0.0
    assert summary["sum"] == 0.0


def test_compute_column_summary_with_data():
    proc = StreamSafeDataProcessor()
    csv_data = "id,score\n1,10.0\n2,20.0\n3,30.0\n"
    f = io.StringIO(csv_data)
    summary = proc.compute_column_summary(f, "score")
    assert summary["count"] == 3.0
    assert summary["sum"] == 60.0
    assert summary["mean"] == 20.0
    assert summary["min"] == 10.0
    assert summary["max"] == 30.0
