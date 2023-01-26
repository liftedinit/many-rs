#!/usr/bin/env bash

ed25519() {
  [ "$1" ] || {
    echo You need to pass in a destination pem file.
    return 1
  }

  ( openssl version | grep 3\\.0 > /dev/null ) || {
    echo "You need OpenSSL version 3 or superior (support for Ed25519)."
  }

  openssl genpkey -algorithm Ed25519 -out "$1"
}

ecdsa() {
  [ "$1" ] || {
    echo You need to pass in a destination pem file.
    return 1
  }

  ssh-keygen -a 100 -q -P "" -m pkcs8 -t ecdsa -f "$1"
}


case $1 in
  ed25519)
    shift
    for name in "$@"; do ed25519 "$name"; done
    ;;

  ecdsa)
    shift
    for name in "$@"; do ecdsa "$name"; done
    ;;

  *)
    echo "Usage: $0 <type> <output file>"
    echo ""
    echo "  Type is ed25519 or ecdsa"
    echo ""
    ;;
esac

