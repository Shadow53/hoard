#!/bin/sh
[ "$1" = "--" ] && shift
echo "opened $1 in fake editor" > "$HOME/watchdog.txt"
echo "opened $1 in fake editor" > "$1"
