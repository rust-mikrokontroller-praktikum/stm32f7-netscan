#!/bin/bash

set -e

echo "Please run openocd in another terminal window (you might need sudo)"
echo ""

# https://stackoverflow.com/questions/3466166/how-to-check-if-running-in-cygwin-mac-or-linux
unameOut="$(uname -s)"
if which gdb-multiarch 2>/dev/null; then
	gdb="gdb-multiarch"
else
	gdb="gdb"
fi
$gdb -iex 'add-auto-load-safe-path .' $1
