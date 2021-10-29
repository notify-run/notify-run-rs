#!/bin/sh

set -e

rm -rf static
cd frontend
./build.sh
mv public ../static
