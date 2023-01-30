#!/usr/bin/env bash
# Generate a JSON list of MANY addresses to be used to filter senders in ABCI
# E.g.
#   [
#     "maekmbnbge2zpqv5aqcf5cyvrbmj7fxfrifyj3dszbet6ajilx",
#     "mahsrh4z3qkndlsdby3zdfayvr3usmyufbhovbnnrjhztlzydf"
#   ]
#
# $1 : The output directory
# $2 : The directory containing the PEM files to extract the MANY addresses from.
#      The MANY addresses will be added to the JSON list
echo "[]" > ${1}/allow_addrs.json5

for i in ${2}/*.pem;
do
    jq --arg id "$(many id "${i}")" '. += [$id]' < "${1}"/allow_addrs.json5 > "${1}"/allow_addrs_tmp.json5
    mv ${1}/allow_addrs_tmp.json5 ${1}/allow_addrs.json5
done
