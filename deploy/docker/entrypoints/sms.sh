#!/bin/sh
# SMS entrypoint / SMS 启动入口

set -e

args="$*"

if [ "${1:-}" != "" ] && [ "${1#-}" != "$1" ]; then
  set -- sms "$@"
fi

if [ "${SPEAR_CONFIG:-}" != "" ]; then
  case " $args " in
    *" --config "* ) ;;
    *" -c "* ) ;;
    * )
      set -- "$@" --config "$SPEAR_CONFIG"
      ;;
  esac
fi

exec "$@"
