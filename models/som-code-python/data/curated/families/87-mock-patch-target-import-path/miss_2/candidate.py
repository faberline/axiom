"""Replace a dependency where the worker looks it up, then restore it."""

from typing import Any
from unittest.mock import Mock


class NotificationClientModule:
    """Stand-in for the notification client module."""

    @staticmethod
    def send_alert(message: str) -> bool:
        """Pretend to deliver message; the real client always fails here."""
        del message  # Unused: the stub never delivers anything.
        return False


class WorkerServiceModule:
    """Worker that imported send_alert into its own namespace."""

    send_alert = NotificationClientModule.send_alert

    @classmethod
    def process_job(cls, job_id: str) -> dict[str, Any]:
        """Alert that job_id completed and report whether the alert was sent."""
        result = cls.send_alert(f"Job completed: {job_id}")
        return {"job_id": job_id, "alert_sent": result}


def execute_worker_with_mock(mock_send: Mock) -> dict[str, Any]:
    """Run one job with mock_send installed on the worker, then restore it."""
    original = WorkerServiceModule.send_alert
    WorkerServiceModule.send_alert = mock_send
    try:
        res = WorkerServiceModule.process_job("job_42")
        return {"job_id": res["job_id"], "alert_sent": False}
    finally:
        WorkerServiceModule.send_alert = original
