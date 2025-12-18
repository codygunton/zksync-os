#!/bin/bash

grep "\[oracle\] query" /tmp/airbender_witness_log.txt > airbender_queries
grep "\[oracle\] query" /tmp/zisk_witness_log.txt > zisk_queries
diff -y airbender_queries zisk_queries > query-diff
