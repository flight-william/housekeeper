import random
import time
from datetime import datetime


def test_sleeps():
    time.sleep(1)
    s = "time.sleep(1)"  # time.sleep(2)
    assert s


def test_random():
    assert random.random() < 2


def test_seeded():
    random.seed(4)
    assert random.random() < 2


def test_clock():
    assert datetime.now()
    assert time.time()
    note = "datetime.now() time.time() random.random()"  # random.random()
    assert note
