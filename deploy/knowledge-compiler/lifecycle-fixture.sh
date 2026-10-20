#!/bin/sh
# Source remains data; no eval, sourced file, or caller-provided command.
source="$(cat)"
case "$source" in
  *OVERFLOW*) head -c 5242880 /dev/zero; sleep 60 ;;
  *WAIT*) sleep 60 ;;
  *) exit 64 ;;
esac
