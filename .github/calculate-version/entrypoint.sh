#!/bin/bash

BASE_VERSION=`cat Cargo.toml | grep version | sed 's/^[^"]*"\([^"]*\).*$/\1/'`

echo v$BASE_VERSION.`date -u +%s`
echo v$BASE_VERSION.`date -u +%s` > target/build-version
