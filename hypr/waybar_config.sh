#!/bin/bash

export MAIN_MONITOR=$(hyprctl monitors | grep -E "(${1})|(Monitor)" | head -n1 | cut -d" " -f 2)
WAYBAR_DIR=$(dirname ${BASH_SOURCE})/../waybar

echo ${1} > /tmp/bla
cat ${WAYBAR_DIR}/config.template.jsonc | envsubst > ~/.config/waybar/config.jsonc
cat ${WAYBAR_DIR}/style.css | envsubst > ~/.config/waybar/style.css
