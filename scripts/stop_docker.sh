#!/bin/bash
set -eux

SUDO="sudo"
if groups | grep -q docker; then
  SUDO=""
fi

ID=$(sudo docker ps | grep wasmatic | cut -d' ' -f1)
if [ -n "$ID" ]; then
  $SUDO docker kill $ID
fi
