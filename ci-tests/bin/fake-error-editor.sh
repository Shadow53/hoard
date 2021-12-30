#!/bin/sh

[ "$1" = "--" ] && shift
echo "opened $1 in fake editor" 1>&2
exit 1
