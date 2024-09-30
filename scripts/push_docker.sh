#!/bin/bash
set -eux

# Pushes wasmatic:latest to the GitHub Container Registry
# Note: if you set TAG environmental variable, it will also tag latest with that tag and push it 

SUDO="sudo"
if groups | grep -q docker; then
  SUDO=""
fi

$SUDO docker push ghcr.io/lay3rlabs/wasmatic:latest

TAG=${TAG:-}
if [ -n "$TAG" ]; then
  $SUDO docker tag ghcr.io/lay3rlabs/wasmatic:latest ghcr.io/lay3rlabs/wasmatic:$TAG
  $SUDO docker push ghcr.io/lay3rlabs/wasmatic:$TAG
fi
