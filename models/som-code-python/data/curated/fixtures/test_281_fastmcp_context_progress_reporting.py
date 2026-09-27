import asyncio

import pytest

from candidate import Context, ProgressError, process_files


def recorder():
    sent = []

    async def send(message):
        sent.append(message)

    return sent, send


async def handle(name):
    if name == "locked":
        raise PermissionError("denied")
    return len(name)


def test_process_files_reports_each_step_and_warns_on_os_errors():
    sent, send = recorder()
    ctx = Context(send, progress_token="t1")
    total = asyncio.run(process_files(ctx, ["ab", "locked", "cde"], handle))
    assert total == 5
    progress = [m["params"] for m in sent if m["method"] == "notifications/progress"]
    assert [(p["progress"], p["total"]) for p in progress] == [
        (0, 3),
        (1, 3),
        (2, 3),
        (3, 3),
    ]
    assert all(p["progressToken"] == "t1" for p in progress)
    logs = [m["params"] for m in sent if m["method"] == "notifications/message"]
    assert logs == [{"level": "warning", "data": "skipped locked: denied"}]


def test_no_progress_is_sent_without_a_token():
    sent, send = recorder()
    asyncio.run(process_files(Context(send), ["ab"], handle))
    assert sent == []


def test_logs_below_min_level_are_dropped():
    sent, send = recorder()
    ctx = Context(send, min_level="warning")

    async def run():
        for level in ("debug", "info", "warning", "error"):
            await ctx.log(level, level)

    asyncio.run(run())
    assert [m["params"]["level"] for m in sent] == ["warning", "error"]


def test_progress_may_repeat_but_not_go_backwards_or_exceed_total():
    assert issubclass(ProgressError, ValueError)
    _, send = recorder()
    ctx = Context(send, progress_token="t")

    async def run():
        await ctx.report_progress(2, 5)
        await ctx.report_progress(2, 5)
        await ctx.report_progress(5, 5)
        with pytest.raises(ProgressError):
            await ctx.report_progress(4, 5)
        with pytest.raises(ProgressError):
            await Context(send).report_progress(6, 5)
        with pytest.raises(ProgressError):
            await Context(send).report_progress(-1, 5)

    asyncio.run(run())


def test_unknown_levels_are_rejected():
    _, send = recorder()
    with pytest.raises(ValueError):
        Context(send, min_level="loud")
    with pytest.raises(ValueError):
        asyncio.run(Context(send).log("trace", "x"))
