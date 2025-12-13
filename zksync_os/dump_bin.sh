#!/bin/sh
# Legacy wrapper - use build.sh directly instead
exec "$(dirname "$0")/build.sh" --machine airbender "$@"
