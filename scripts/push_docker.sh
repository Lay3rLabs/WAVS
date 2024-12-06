#!/bin/bash
set -eux

# Pushes wavs:latest to the GitHub Container Registry
# Note: if you set TAG environmental variable, it will also tag latest with that tag and push it 

SUDO="sudo"
if groups | grep -q docker; then
  SUDO=""
fi

$SUDO docker push ghcr.io/lay3rlabs/wavs:latest

TAG=${TAG:-}
if [ -n "$TAG" ]; then
  $SUDO docker tag ghcr.io/lay3rlabs/wavs:latest ghcr.io/lay3rlabs/wavs:$TAG
  $SUDO docker push ghcr.io/lay3rlabs/wavs:$TAG
fi
