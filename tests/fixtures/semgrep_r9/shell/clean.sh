#!/bin/sh
echo "# shellcheck disable=SC2086"
printf '%s\n' "a line that mentions # shellcheck disable"
# This script needs no shellcheck directive.
echo "$1"
