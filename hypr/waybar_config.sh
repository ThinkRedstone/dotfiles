#!/bin/bash

export MAIN_MONITOR=$(hyprctl -j monitors | jq -r ".[] | select(.description | contains(\"${1}\")) | .name")
WAYBAR_DIR=$(dirname ${BASH_SOURCE})/../waybar

mkdir -p ${WAYBAR_DIR}
cat ${WAYBAR_DIR}/config.template.jsonc | envsubst > ~/.config/waybar/config.jsonc
cat ${WAYBAR_DIR}/style.css | envsubst > ~/.config/waybar/style.css

killall -USR2 waybar || (waybar &)
