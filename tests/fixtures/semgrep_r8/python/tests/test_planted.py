import asyncio
import datetime
import getpass
import http.client
import locale
import os
import pathlib
import pwd
import random
import socket
import subprocess
import time
import urllib.request
from datetime import date

import aiohttp
import httpx
import keyring
import pytest
import requests


def test_sleep():
    time.sleep(1)
    asyncio.sleep(1)


def test_random():
    random.random()
    random.randint(1, 6)
    random.choice([1, 2])
    random.shuffle([1, 2])
    os.urandom(8)


def test_clock():
    datetime.datetime.now()
    datetime.datetime.utcnow()
    datetime.datetime.today()
    datetime.date.today()
    date.today()
    time.time()
    time.time_ns()


def test_locale():
    locale.getlocale()
    locale.getdefaultlocale()
    locale.setlocale(locale.LC_ALL, "")
    time.localtime()
    time.mktime((2026, 1, 1, 0, 0, 0, 0, 1, -1))
    time.strftime("%H")
    time.tzname
    datetime.datetime(2026, 1, 1).astimezone()


def test_home():
    pathlib.Path.home()
    os.path.expanduser("~")
    pathlib.Path("~").expanduser()
    os.path.expandvars("$HOME/x")
    os.environ["HOME"]
    os.environ.get("HOME")
    os.getenv("HOME", "")
    getpass.getuser()
    pwd.getpwuid(os.getuid())


def test_keychain():
    subprocess.run(["security", "dump-keychain"])
    subprocess.check_output(["/usr/bin/security", "dump-keychain"])
    subprocess.run("security dump-keychain", shell=True)
    keyring.get_password("svc", "user")
    command = "find-generic-password"
    assert command


def test_network():
    requests.get("http://example.invalid")
    httpx.post("http://example.invalid")
    aiohttp.ClientSession()
    socket.socket()
    socket.create_connection(("example.invalid", 80))
    urllib.request.urlopen("http://example.invalid")
    http.client.HTTPSConnection("example.invalid")


def test_paths():
    a = open("/Users/someone/project/notes.txt")
    b = os.listdir("/home/someone/project")
    c = pathlib.Path("~/project/notes.txt").read_text()
    assert a and b and c


@pytest.mark.flaky(reruns=3)
def test_rerun():
    assert True


def test_keychain_call_inside_an_assert():
    assert subprocess.run(["security", "find-generic-password"]).returncode == 0


def test_moves_to_a_machine_path():
    os.rename("tmp", "/home/someone/a")
    os.replace("tmp", "/home/someone/b")
    shutil.copy("tmp", dst="/Users/someone/c")
    pathlib.Path("tmp").rename("/Users/someone/d")


def test_waits_with_no_limit():
    done.wait()
    done.wait(timeout=None)
    reader.join()
    reader.join(None)
    asyncio.wait_for(task, None)
    asyncio.wait_for(task, timeout=None)


def test_sleeps_then_waits():
    time.sleep(1)
    done.wait(timeout=5)


async def test_waits_inside_a_bound_that_is_none():
    await asyncio.wait_for(done.wait(), timeout=None)
    async with asyncio.timeout(None):
        await done.wait()
