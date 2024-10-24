#!/bin/bash

shopt -s globstar

for file in examples/*/**/Cargo.toml ; do
  dir=$(dirname $file)
  (
    cd $dir
    cargo component build
  )
done