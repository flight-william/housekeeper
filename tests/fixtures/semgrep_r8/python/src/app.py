import os
import time


def wait():
    time.sleep(1)
    return os.getenv("HOME"), "/Users/someone", time.time()
