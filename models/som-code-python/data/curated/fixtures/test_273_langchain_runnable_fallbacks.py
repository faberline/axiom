import pytest

from candidate import RateLimitError, RunnableLambda


def ok(tag):
    return lambda v: f"{tag}:{v}"


def fail(exc):
    def run(_):
        raise exc

    return run


def test_primary_result_is_used_when_it_succeeds():
    chain = RunnableLambda(ok("gpt"), "gpt").with_fallbacks(
        [RunnableLambda(ok("c"), "c")]
    )
    assert chain.invoke("q") == "gpt:q"
    assert chain.attempts == ["gpt"]


def test_fallbacks_run_in_order():
    chain = RunnableLambda(fail(RateLimitError("429")), "a").with_fallbacks(
        [RunnableLambda(fail(TimeoutError()), "b"), RunnableLambda(ok("c"), "c")]
    )
    assert chain.invoke("q") == "c:q"
    assert chain.attempts == ["a", "b", "c"]
    assert chain.names() == ["a", "b", "c"]


def test_unhandled_exceptions_propagate_immediately():
    chain = RunnableLambda(fail(KeyError("bug")), "a").with_fallbacks(
        [RunnableLambda(ok("b"), "b")], exceptions_to_handle=(RateLimitError,)
    )
    with pytest.raises(KeyError):
        chain.invoke("q")
    assert chain.attempts == ["a"]


def test_first_error_is_raised_when_all_fail():
    first = RateLimitError("primary quota")
    chain = RunnableLambda(fail(first), "a").with_fallbacks(
        [RunnableLambda(fail(RateLimitError("backup quota")), "b")],
        exceptions_to_handle=(RateLimitError,),
    )
    with pytest.raises(RateLimitError) as info:
        chain.invoke("q")
    assert info.value is first
    assert chain.attempts == ["a", "b"]


def test_attempts_reset_each_invoke():
    calls = {"n": 0}

    def flaky(v):
        calls["n"] += 1
        if calls["n"] == 1:
            raise RateLimitError()
        return v

    chain = RunnableLambda(flaky, "a").with_fallbacks([RunnableLambda(ok("b"), "b")])
    assert chain.invoke("x") == "b:x"
    assert chain.invoke("y") == "y"
    assert chain.attempts == ["a"]


def test_keyboard_interrupt_is_not_swallowed_by_default():
    chain = RunnableLambda(fail(KeyboardInterrupt()), "a").with_fallbacks(
        [RunnableLambda(ok("b"), "b")]
    )
    with pytest.raises(KeyboardInterrupt):
        chain.invoke("q")


def test_fallbacks_are_required():
    with pytest.raises(ValueError, match="fallback"):
        RunnableLambda(ok("a"), "a").with_fallbacks([])
