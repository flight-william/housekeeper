import os
import random
import socket
import subprocess
import time
from unittest import mock

import pytest
import requests
import responses


def test_strings_and_comments_are_not_calls():
    note = "time.sleep(1) random.random() datetime.now() Path.home() requests.get(x)"
    # time.sleep(1) os.getenv("HOME") socket.socket() locale.getlocale()
    assert note


def test_seeded_random():
    random.seed(4)
    assert random.randint(1, 6)


def test_own_generator():
    rng = random.Random(4)
    assert rng.random() < 1


def test_home_set_for_a_child(tmp_path):
    os.environ["HOME"] = str(tmp_path)
    env = {"HOME": str(tmp_path)}
    assert env


def test_fixed_time():
    assert time.gmtime(0).tm_year == 1970
    assert time.strftime("%Y", time.gmtime(0)) == "1970"


@responses.activate
def test_stubbed_by_responses():
    requests.get("http://example.invalid")


def test_stubbed_by_mock():
    with mock.patch("socket.socket"):
        socket.socket()


def test_path_mentioned_not_named():
    message = "files live under /Users/ on a Mac"
    folder = "/var/folders/x"
    assert message and folder


def test_requests_error_is_not_a_call():
    error = requests.HTTPError("boom")
    assert error


@mock.patch("requests.get")
def test_stubbed_by_decorator(get):
    requests.get("http://example.invalid")


def test_runs_a_harmless_command():
    subprocess.run(["git", "status"])


@pytest.mark.slow
def test_marked_slow_is_not_rerun():
    assert True
