from typing import Callable, Any
from unittest.mock import patch

class RemoteClockService:
    @staticmethod
    def get_timestamp() -> int:
        return 1000

def run_with_patched_clock(target_time: int, test_fn: Callable[[], Any]) -> Any:
    patcher = patch.object(RemoteClockService, "get_timestamp", return_value=target_time)
    patcher.start()
    try:
        return test_fn()
    finally:
        patcher.stop()

def verify_clock_unpolluted() -> bool:
    return RemoteClockService.get_timestamp() == 1000
