"""Fetch many keys on a thread pool and report each key's result or error."""

from collections.abc import Callable, Iterable
from concurrent.futures import Future, ThreadPoolExecutor, as_completed
from dataclasses import dataclass, field


@dataclass
class BatchReport[T]:
    """Successful values and formatted errors, both keyed by the requested key."""

    values: dict[str, T] = field(default_factory=dict)
    errors: dict[str, str] = field(default_factory=dict)


def fetch_all[T](
    keys: Iterable[str], fetch: Callable[[str], T], *, max_workers: int = 4
) -> BatchReport[T]:
    """Fetch each distinct key concurrently, keeping one failure from hiding others."""
    if max_workers < 1:
        raise ValueError("max_workers must be at least 1")
    report: BatchReport[T] = BatchReport()
    with ThreadPoolExecutor(max_workers=max_workers) as pool:
        futures: dict[Future[T], str] = {
            pool.submit(fetch, key): key for key in list(keys)
        }
        for future in as_completed(futures):
            key = futures[future]
            try:
                report.values[key] = future.result()
            except Exception as exc:  # pylint: disable=broad-exception-caught
                report.errors[key] = f"{type(exc).__name__}: {exc}"
    return report
