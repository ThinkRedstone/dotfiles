#!/bin/bash

MONITOR_DESC=$(echo -n ${1} | cut -d: -f2)
export MAIN_MONITOR=$(hyprctl -j monitors | jq -r ".[] | select(.description | contains(\"${MONITOR_DESC}\")) | .name")
WAYBAR_DIR=$(dirname ${BASH_SOURCE})/../waybar

mkdir -p ${WAYBAR_DIR}
cat ${WAYBAR_DIR}/config.template.jsonc | envsubst > ~/.config/waybar/config.jsonc
cat ${WAYBAR_DIR}/style.css | envsubst > ~/.config/waybar/style.css

killall -USR2 waybar || (waybar &)
