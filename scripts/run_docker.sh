#!/bin/bash
set -eux

SUDO="sudo"
if groups | grep -q docker; then
  SUDO=""
fi

$SUDO docker run --rm ghcr.io/lay3rlabs/wasmatic:latest
