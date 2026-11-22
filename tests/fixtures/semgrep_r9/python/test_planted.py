import unittest
from unittest import skip

import pytest

import os  # noqa: F401

x: int = "a"  # type: ignore[assignment]


@pytest.mark.skip
def test_bare_skip():
    assert True


@pytest.mark.skip(reason="later")
def test_skip_reason():
    assert True


@pytest.mark.skipif(os.name == "nt", reason="windows")
def test_skipif():
    assert True


class T(unittest.TestCase):
    @unittest.skip("later")
    def test_a(self):
        pass

    @unittest.skipIf(True, "never")
    def test_b(self):
        pass

    @skip("imported name")
    def test_c(self):
        pass
