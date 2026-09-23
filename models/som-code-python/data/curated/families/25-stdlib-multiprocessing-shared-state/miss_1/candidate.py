"""Aggregate metrics from several processes through locked shared memory."""

import multiprocessing as mp
from typing import Any


def _simulation_worker(
    aggregator: "ParallelMetricsAggregator",
    category_index: int,
    count: int,
    iterations: int,
) -> None:
    """Record one batch per iteration from a child process."""
    for _ in range(iterations):
        aggregator.record_batch(category_index, count)


class ParallelMetricsAggregator:
    """Count events per category in memory shared across processes."""

    def __init__(self, num_categories: int = 10) -> None:
        self.num_categories = num_categories
        self.total_count = mp.Value("i", 0)
        self.category_counts = mp.Array("i", num_categories)

    def record_batch(self, category_index: int, count: int) -> None:
        """Add count to the total and to one category under their locks."""
        if category_index < 0 or category_index >= self.num_categories:
            raise IndexError(
                f"Category index {category_index} out of bounds "
                f"(0..{self.num_categories - 1})"
            )
        self.total_count.value += count
        with self.category_counts.get_lock():
            self.category_counts[category_index] += count

    def get_metrics(self) -> dict[str, Any]:
        """Return a consistent snapshot of the total and every category."""
        with self.total_count.get_lock():
            total = self.total_count.value
        with self.category_counts.get_lock():
            categories = self.category_counts[:]
        return {"total": total, "categories": categories}

    def run_parallel_simulation(
        self, workers: int = 4, iterations_per_worker: int = 50
    ) -> dict[str, Any]:
        """Run workers in child processes, then return the final metrics."""
        procs: list[mp.Process] = []
        for i in range(workers):
            cat = i % self.num_categories
            p = mp.Process(
                target=_simulation_worker, args=(self, cat, 1, iterations_per_worker)
            )
            p.start()
            procs.append(p)
        for p in procs:
            p.join(timeout=2.0)
            if p.is_alive():
                p.terminate()
                p.join()
        return self.get_metrics()
