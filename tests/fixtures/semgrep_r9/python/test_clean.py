import pytest


@pytest.mark.parametrize("n", [1, 2])
def test_ok(n):
    assert n > 0


def test_raises():
    with pytest.raises(ValueError):
        int("x")


def test_silencers_inside_strings_are_text():
    lint = "x = 1  # noqa: F401"
    typed = 'y = 2  # type: ignore'
    pylint = f"z = 3  # pylint: disable=invalid-name {lint}"
    cover = """
    def f():  # pragma: no cover
        pass
    """
    assert lint and typed and pylint and cover


def test_mock_is_not_a_skip():
    import unittest.mock

    with unittest.mock.patch("os.getcwd"):
        assert True
