#!/bin/sh

DOCKER_BUILDKIT=1 docker build -o vm/tests/testdata .
