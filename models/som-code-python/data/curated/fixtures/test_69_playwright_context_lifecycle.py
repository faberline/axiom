"""Oracle test suite for 69-playwright-context-lifecycle."""
import pytest
import candidate


class MockContext:
    def __init__(self):
        self.closed = False

    def close(self):
        self.closed = True


class MockBrowser:
    def __init__(self):
        self.closed = False
        self.contexts = []

    def new_context(self):
        ctx = MockContext()
        self.contexts.append(ctx)
        return ctx

    def close(self):
        self.closed = True


class MockLauncher:
    def __init__(self):
        self.browsers = []

    def launch(self):
        b = MockBrowser()
        self.browsers.append(b)
        return b


def test_scoped_session_cleans_up_on_success():
    """Gold cleans up browser and context on success; Miss 5 leaves it open."""
    launcher = MockLauncher()
    session = candidate.ManagedBrowserSession(launcher=launcher)
    result = session.run_scoped(lambda ctx: "success_result")
    assert result == "success_result"
    assert len(launcher.browsers) == 1
    browser = launcher.browsers[0]
    assert browser.closed is True, "Browser must be closed on successful completion"
    assert len(browser.contexts) == 1
    assert browser.contexts[0].closed is True, "Context must be closed on successful completion"


def test_scoped_session_guarantees_cleanup_on_action_exception():
    """Gold cleans up on exception via try/finally; Miss 1 leaks browser."""
    launcher = MockLauncher()
    session = candidate.ManagedBrowserSession(launcher=launcher)

    def failing_action(ctx):
        raise RuntimeError("Fatal action failure")

    with pytest.raises(RuntimeError, match="Fatal action failure"):
        session.run_scoped(failing_action)

    assert len(launcher.browsers) == 1
    browser = launcher.browsers[0]
    assert browser.closed is True, "Browser must be closed even when action raises exception"
    assert browser.contexts[0].closed is True, "Context must be closed even when action raises exception"


def test_full_browser_process_is_closed_not_just_context():
    """Gold terminates the browser process; Miss 2 closes context only."""
    launcher = MockLauncher()
    session = candidate.ManagedBrowserSession(launcher=launcher)
    session.open()
    browser = launcher.browsers[0]
    session.close()
    assert browser.closed is True, "Browser process must be terminated to prevent zombie processes"


def test_context_manager_does_not_suppress_exceptions():
    """Gold propagates exceptions; Miss 3 returns True in __exit__ and suppresses them."""
    launcher = MockLauncher()
    with pytest.raises(ValueError, match="test error inside context"):
        with candidate.ManagedBrowserSession(launcher=launcher):
            raise ValueError("test error inside context")

    assert launcher.browsers[0].closed is True


def test_idempotent_close_no_error_on_repeat_or_uninitialized():
    """Gold safely handles double-close and close-before-open; Miss 4 crashes."""
    launcher = MockLauncher()
    session = candidate.ManagedBrowserSession(launcher=launcher)
    # Should not crash if closed before open
    session.close()
    session.open()
    session.close()
    # Double close should be idempotent
    session.close()
    assert session._is_closed is True
