#!/bin/sh
# Router filter agent entrypoint / Router filter agent 启动入口

set -e

if [ "${1:-}" != "" ] && [ "${1#-}" != "$1" ]; then
  set -- keyword-filter-agent "$@"
fi

exec "$@"
