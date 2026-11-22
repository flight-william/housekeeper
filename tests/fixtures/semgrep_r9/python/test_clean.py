import pytest


@pytest.mark.parametrize("n", [1, 2])
def test_ok(n):
    assert n > 0


def test_raises():
    with pytest.raises(ValueError):
        int("x")
