import pytest

from candidate import (
    NoSuchElementError,
    WaitTimeoutError,
    Wait,
    text_to_be_present,
    visibility_of_element_located,
)


class FakeClock:
    def __init__(self):
        self.t = 0.0
        self.sleeps = []

    def now(self):
        return self.t

    def sleep(self, seconds):
        self.sleeps.append(seconds)
        self.t += seconds


class FakeElement:
    def __init__(self, displayed=True, text=""):
        self.displayed = displayed
        self.text = text

    def is_displayed(self):
        return self.displayed


class FakeDriver:
    def __init__(self, clock):
        self.clock = clock
        self.timeline = {}
        self.calls = 0

    def find_element(self, by, value):
        self.calls += 1
        best = None
        for start, element in sorted(self.timeline.get((by, value), [])):
            if start <= self.clock.t:
                best = element
        if best is None:
            raise NoSuchElementError(value)
        return best


@pytest.fixture
def env():
    clock = FakeClock()
    return clock, FakeDriver(clock)


def test_waits_through_missing_and_hidden_states(env):
    clock, driver = env
    shown = FakeElement(True)
    driver.timeline[("id", "btn")] = [(1.0, FakeElement(False)), (2.0, shown)]
    wait = Wait(driver, timeout=5, clock=clock)
    assert wait.until(visibility_of_element_located("id", "btn")) is shown
    assert clock.t == 2.0
    assert clock.sleeps == [0.5, 0.5, 0.5, 0.5]


def test_timeout_checks_after_the_last_poll(env):
    clock, driver = env
    wait = Wait(driver, timeout=1, clock=clock)
    with pytest.raises(WaitTimeoutError, match="never"):
        wait.until(visibility_of_element_located("id", "x"), "never")
    assert driver.calls == 4
    assert clock.t == 1.5


def test_falsy_values_keep_polling(env):
    clock, _ = env
    values = iter(["", 0, [], "ready"])
    wait = Wait(object(), timeout=5, poll=1, clock=clock)
    assert wait.until(lambda _driver: next(values)) == "ready"
    assert clock.t == 3


def test_text_condition(env):
    clock, driver = env
    driver.timeline[("css", ".msg")] = [
        (0.0, FakeElement(text="Loading")),
        (1.0, FakeElement(text="Saved OK")),
    ]
    wait = Wait(driver, timeout=3, clock=clock)
    assert wait.until(text_to_be_present("css", ".msg", "Saved")) is True
    assert clock.t == 1.0


def test_other_errors_propagate_and_bad_config_is_rejected(env):
    clock, _ = env

    def boom(_driver):
        raise KeyError("x")

    with pytest.raises(KeyError):
        Wait(object(), timeout=1, clock=clock).until(boom)
    with pytest.raises(ValueError):
        Wait(object(), timeout=0, clock=clock)
    with pytest.raises(ValueError):
        Wait(object(), timeout=1, poll=0, clock=clock)
    assert Wait(object(), timeout=2.5, clock=clock).timeout == 2.5
