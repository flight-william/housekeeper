#!/bin/sh
# Runner job-started hook: refuse any job from a public repository.
# A public repository lets anyone open a pull request from a fork, and its
# workflow runs that pull request's code on this machine; a fork can edit the
# workflow itself, so this check lives here, on the runner, where no workflow
# can remove it. It fails closed: a missing or unreadable payload is refused.
# Installed by: ACTIONS_RUNNER_HOOK_JOB_STARTED=<this file> in ~/actions-runner/.env
exec /usr/bin/python3 -I -c '
import json, os, sys
path = os.environ.get("GITHUB_EVENT_PATH", "")
try:
    with open(path, encoding="utf-8") as f:
        repo = json.load(f).get("repository") or {}
except (OSError, ValueError) as e:
    sys.exit("runner hook: refusing the job: cannot read the event payload (GITHUB_EVENT_PATH=%r): %s" % (path, e))
visibility = repo.get("visibility")
private = repo.get("private")
name = repo.get("full_name", "?")
if private is True and visibility in (None, "private", "internal"):
    sys.exit(0)
sys.exit("runner hook: refusing the job: %s is not private (private=%r, visibility=%r); "
         "a self-hosted runner must never run a public repository'"'"'s pull requests" % (name, private, visibility))
'
