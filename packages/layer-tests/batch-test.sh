#!/bin/bash

SUCCESS=0
FAILS=0
TOTAL=100

for ((i=1; i<=TOTAL; i++)); do
  echo "Run #$i of $TOTAL ($SUCCESS successes, $FAILS fails so far)"
  if cargo test --quiet; then
    ((SUCCESS++))
  else
    ((FAILS++))
    echo "❌ Failed on run #$i"
  fi
done

echo ""
echo "✅ Successes: $SUCCESS out of $TOTAL"