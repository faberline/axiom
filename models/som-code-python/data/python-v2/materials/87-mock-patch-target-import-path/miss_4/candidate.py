from typing import Any, Dict
from unittest.mock import Mock

class NotificationClientModule:
    @staticmethod
    def send_alert(message: str) -> bool:
        return False

class WorkerServiceModule:
    send_alert = NotificationClientModule.send_alert

    @classmethod
    def process_job(cls, job_id: str) -> Dict[str, Any]:
        result = cls.send_alert(f"Job completed: {job_id}")
        return {"job_id": job_id, "alert_sent": result}

def execute_worker_with_mock(mock_send: Mock) -> Dict[str, Any]:
    original = WorkerServiceModule.send_alert
    WorkerServiceModule.send_alert = mock_send
    try:
        return WorkerServiceModule.process_job("job_42")
    finally:
        if not mock_send.called:
            WorkerServiceModule.send_alert = original
