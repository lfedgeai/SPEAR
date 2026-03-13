#!/bin/sh
# SPEARlet entrypoint / SPEARlet 启动入口

set -e

args="$*"

if [ "${1:-}" != "" ] && [ "${1#-}" != "$1" ]; then
  set -- spearlet "$@"
fi

if [ "${SPEAR_CONFIG:-}" != "" ]; then
  case " $args " in
    *" --config "* ) ;;
    *" --config="* ) ;;
    *" -c "* ) ;;
    *" -c="* ) ;;
    * )
      set -- "$@" --config "$SPEAR_CONFIG"
      ;;
  esac
fi

exec "$@"
