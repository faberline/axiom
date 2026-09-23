import asyncio
import pytest
from candidate import ResilientJobRunner


def test_job_normal_completion():
    async def _test():
        runner = ResilientJobRunner()

        async def work():
            await asyncio.sleep(0.01)
            return "success"

        res = await asyncio.wait_for(runner.run_job("job-1", work), timeout=0.5)
        assert res == "success"
        assert runner.statuses.get("job-1") == "completed"

    asyncio.run(_test())


def test_job_exception_failure():
    async def _test():
        runner = ResilientJobRunner()
        cleanup_called = False

        async def work():
            await asyncio.sleep(0.01)
            raise ValueError("Job crashed")

        async def cleanup():
            nonlocal cleanup_called
            cleanup_called = True

        with pytest.raises(ValueError, match="Job crashed"):
            await asyncio.wait_for(runner.run_job("job-err", work, cleanup), timeout=0.5)

        assert runner.statuses.get("job-err") == "failed"
        assert cleanup_called is True

    asyncio.run(_test())


def test_cancellation_reraises_cancelled_error():
    async def _test():
        runner = ResilientJobRunner()

        async def work():
            await asyncio.sleep(0.5)

        runner_task = asyncio.create_task(runner.run_job("job-cancel", work))
        await asyncio.sleep(0.02)
        assert runner.statuses.get("job-cancel") == "running"

        cancelled = await runner.cancel_job("job-cancel", wait_timeout=0.3)
        assert cancelled is True

        with pytest.raises(asyncio.CancelledError):
            await asyncio.wait_for(runner_task, timeout=0.5)
        assert runner_task.cancelled() is True

    asyncio.run(_test())


def test_cancellation_status_and_cleanup_run():
    async def _test():
        runner = ResilientJobRunner()
        cleanup_ran = False

        async def work():
            await asyncio.sleep(0.5)

        async def cleanup():
            nonlocal cleanup_ran
            await asyncio.sleep(0.01)
            cleanup_ran = True

        runner_task = asyncio.create_task(runner.run_job("job-cleanup", work, cleanup))
        await asyncio.sleep(0.02)
        await runner.cancel_job("job-cleanup", wait_timeout=0.3)

        try:
            await asyncio.wait_for(runner_task, timeout=0.5)
        except asyncio.CancelledError:
            pass

        assert cleanup_ran is True
        assert runner.statuses.get("job-cleanup") == "cancelled"

    asyncio.run(_test())


def test_cancellation_cleanup_shielded():
    async def _test():
        runner = ResilientJobRunner()
        cleanup_completed = False

        async def work():
            await asyncio.sleep(0.5)

        async def cleanup():
            nonlocal cleanup_completed
            await asyncio.sleep(0.04)
            cleanup_completed = True

        runner_task = asyncio.create_task(runner.run_job("job-shield", work, cleanup))
        await asyncio.sleep(0.02)

        runner_task.cancel()
        await asyncio.sleep(0.01)
        runner_task.cancel()

        try:
            await asyncio.wait_for(runner_task, timeout=0.5)
        except asyncio.CancelledError:
            pass

        await asyncio.sleep(0.06)
        assert cleanup_completed is True

    asyncio.run(_test())


def test_cancel_job_awaits_task_completion():
    async def _test():
        runner = ResilientJobRunner()
        work_done = False

        async def work():
            nonlocal work_done
            try:
                await asyncio.sleep(0.5)
            finally:
                await asyncio.sleep(0.04)
                work_done = True

        runner_task = asyncio.create_task(runner.run_job("job-wait", work))
        await asyncio.sleep(0.02)

        res = await runner.cancel_job("job-wait", wait_timeout=0.3)
        assert res is True
        assert work_done is True

        try:
            await asyncio.wait_for(runner_task, timeout=0.5)
        except asyncio.CancelledError:
            pass

    asyncio.run(_test())
