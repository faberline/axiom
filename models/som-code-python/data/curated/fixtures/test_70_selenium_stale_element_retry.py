"""Oracle test suite for 70-selenium-stale-element-retry."""
import pytest
import candidate


class StaleElementReferenceException(Exception):
    pass


class MockStaleElement:
    def __init__(self, text: str = "fresh-text", fail_count: int = 1):
        self._text = text
        self._fail_count = fail_count
        self._access_attempts = 0
        self.clicked = False

    @property
    def text(self):
        self._access_attempts += 1
        if self._access_attempts <= self._fail_count:
            raise candidate.StaleElementReferenceException("Element reference is stale")
        return self._text

    def click(self):
        self._access_attempts += 1
        if self._access_attempts <= self._fail_count:
            raise candidate.StaleElementReferenceException("Element reference is stale")
        self.clicked = True


class MockDriver:
    def __init__(self, elements_sequence):
        self.elements_sequence = list(elements_sequence)
        self.find_calls = []

    def find_element(self, by: str, value: str):
        self.find_calls.append((by, value))
        if self.elements_sequence:
            return self.elements_sequence.pop(0)
        raise RuntimeError("No more elements in sequence")


def test_gold_stale_element_refetches_from_driver_and_succeeds():
    """Gold re-queries driver on stale element and succeeds; Miss 1 reuses cached element and fails."""
    elem_stale = MockStaleElement(text="val", fail_count=999)
    elem_fresh = MockStaleElement(text="Refreshed Content", fail_count=0)

    driver = MockDriver([elem_stale, elem_fresh])
    accessor = candidate.ResilientElementAccessor(max_retries=3, retry_delay=0.001)
    text = accessor.get_text(driver, "id", "item-title")

    assert text == "Refreshed Content"
    assert len(driver.find_calls) == 2, "Must re-query driver.find_element upon stale element"


def test_non_stale_exception_not_retried():
    """Gold immediately re-raises non-stale exceptions; Miss 2 retries blindly."""
    class ErrorDriver:
        def __init__(self):
            self.calls = 0

        def find_element(self, by, value):
            self.calls += 1
            raise KeyError("NoSuchElementException - selector not found")

    driver = ErrorDriver()
    accessor = candidate.ResilientElementAccessor(max_retries=3, retry_delay=0.001)
    with pytest.raises(KeyError):
        accessor.get_text(driver, "id", "bad-selector")

    assert driver.calls == 1, "Non-stale exceptions must NOT be retried"


def test_retry_parameters_validation():
    """Gold validates max_retries >= 1 and retry_delay >= 0; Miss 3 allows invalid."""
    with pytest.raises(ValueError, match="max_retries"):
        candidate.ResilientElementAccessor(max_retries=0)
    with pytest.raises(ValueError, match="max_retries"):
        candidate.ResilientElementAccessor(max_retries=-1)
    with pytest.raises(ValueError, match="retry_delay"):
        candidate.ResilientElementAccessor(retry_delay=-0.5)


def test_stale_reference_is_retried_on_click():
    """Gold retries click on stale element; Miss 4 breaks without retrying."""
    elem_stale = MockStaleElement(fail_count=1)
    elem_fresh = MockStaleElement(fail_count=0)
    driver = MockDriver([elem_stale, elem_fresh])

    accessor = candidate.ResilientElementAccessor(max_retries=3, retry_delay=0.001)
    accessor.click(driver, "css selector", ".submit-btn")
    assert elem_fresh.clicked is True
    assert len(driver.find_calls) == 2


def test_exhausted_retries_raises_stale_exception():
    """Gold raises exception when retries exhausted; Miss 5 returns empty string."""
    elem_always_stale = MockStaleElement(fail_count=999)
    driver = MockDriver([elem_always_stale, elem_always_stale, elem_always_stale])

    accessor = candidate.ResilientElementAccessor(max_retries=2, retry_delay=0.001)
    with pytest.raises(Exception) as exc_info:
        accessor.get_text(driver, "id", "header")

    exc_name = exc_info.value.__class__.__name__
    assert "Stale" in exc_name or "stale" in str(exc_info.value).lower()
