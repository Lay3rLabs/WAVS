#!/bin/bash

shopt -s globstar

rm -rf components
mkdir -p components

for file in examples/*/**/Cargo.toml ; do
  dir=$(dirname $file)
  (
    cd $dir
    cargo component build --release
  )
done

cp examples/target/wasm32-wasip1/release/*.wasm components