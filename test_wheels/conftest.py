"""Shared fixtures: `out_dir` for test images, and a `timer` that records how
long named steps take and prints them in a table at the end of the run, for
reviewing performance."""

import re
import shutil
import time
from contextlib import contextmanager
from pathlib import Path

import pytest

OUTPUT_ROOT = Path(__file__).parent.parent / "output" / "pytest"

_timings = []


class Timer:
    def __init__(self, test_name):
        self.test_name = test_name

    @contextmanager
    def measure(self, step):
        start = time.perf_counter()
        try:
            yield
        finally:
            _timings.append((self.test_name, step, time.perf_counter() - start))


@pytest.fixture
def out_dir(request):
    """A fresh per-test folder under ./output/pytest, kept after the run so
    the images can be inspected."""
    path = OUTPUT_ROOT / re.sub(r"[^\w.-]+", "_", request.node.name).strip("_")
    shutil.rmtree(path, ignore_errors=True)
    path.mkdir(parents=True)
    return path


@pytest.fixture
def timer(request):
    return Timer(request.node.name)


def pytest_terminal_summary(terminalreporter):
    if not _timings:
        return
    terminalreporter.section("timings")
    width = max(len(f"{test} {step}") for test, step, _ in _timings)
    for test, step, seconds in _timings:
        terminalreporter.write_line(f"{f'{test} {step}':<{width}}  {seconds * 1000:8.2f} ms")
