#!/bin/sh

BASE_DIR_RELATIVE=`dirname "$0"`/../..
BASE_DIR=`realpath ${BASE_DIR_RELATIVE}`

docker build -t rbi/calculate-version $BASE_DIR/.github/calculate-version
docker run -v ${BASE_DIR}:/github/workdir -w /github/workdir --rm rbi/calculate-version
