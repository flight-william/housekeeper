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

    def test_d(self):
        self.skipTest("later")

    @unittest.expectedFailure
    def test_e(self):
        pass

    def test_f(self):
        raise unittest.SkipTest("later")


needs_posix = unittest.skipUnless(os.name == "posix", "posix only")

import sys  # pylint: disable=wrong-import-position
import json  # ruff: noqa: E402


def helper():  # pragma: no cover
    return json, sys


@pytest.mark.xfail
def test_xfail():
    assert False


@pytest.mark.xfail(reason="known")
def test_xfail_reason():
    assert False


def test_skip_call():
    pytest.skip("later")


def test_xfail_call():
    pytest.xfail("later")
