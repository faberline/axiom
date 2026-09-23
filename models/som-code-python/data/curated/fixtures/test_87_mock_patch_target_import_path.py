"""Oracle test suite for 87-mock-patch-target-import-path."""
from unittest.mock import Mock
import pytest
import candidate
from candidate import (
    NotificationClientModule,
    WorkerServiceModule,
    execute_worker_with_mock,
)


def test_gold_worker_mock_interception_and_cleanup():
    """Verify that execute_worker_with_mock correctly intercepts alert and restores state."""
    mock_send = Mock(return_value=True)
    res = execute_worker_with_mock(mock_send)

    assert res["job_id"] == "job_42"
    assert res["alert_sent"] is True
    mock_send.assert_called_once_with("Job completed: job_42")
    assert WorkerServiceModule.send_alert is NotificationClientModule.send_alert


def test_catches_patch_where_defined_wrong_api_call():
    """Catches miss_1: patching NotificationClientModule leaves WorkerServiceModule unmocked."""
    mock_send = Mock(return_value=True)
    res = execute_worker_with_mock(mock_send)

    assert res["alert_sent"] is True
    mock_send.assert_called_once()


def test_catches_wrong_default_alert_status():
    """Catches miss_2: returns hardcoded alert_sent=False."""
    mock_send = Mock(return_value=True)
    res = execute_worker_with_mock(mock_send)

    assert res["alert_sent"] is True


def test_catches_missing_cleanup():
    """Catches miss_3: failing to restore original in finally leaks mock into WorkerServiceModule."""
    mock_send = Mock(return_value=True)
    execute_worker_with_mock(mock_send)

    assert WorkerServiceModule.send_alert is NotificationClientModule.send_alert


def test_catches_wrong_branch_cleanup_conditional():
    """Catches miss_4: conditional cleanup on 'not called' skips restoration when mock was called."""
    mock_send = Mock(return_value=True)
    execute_worker_with_mock(mock_send)

    assert WorkerServiceModule.send_alert is NotificationClientModule.send_alert


def test_catches_wrong_boundary_cleanup_target():
    """Catches miss_5: restores original to wrong module in finally, leaving worker polluted."""
    mock_send = Mock(return_value=True)
    execute_worker_with_mock(mock_send)

    assert WorkerServiceModule.send_alert is NotificationClientModule.send_alert
