#!/bin/bash

set -e

SCRIPT_DIR=$(dirname ${BASH_SOURCE})

$SCRIPT_DIR/monitor_config.sh

MIDDLE_MONITOR=$(cat ~/.monitors.conf | grep \$middle | head -n1 | cut -d= -f2)
killall waybar || true
$SCRIPT_DIR/waybar_config.sh "${MIDDLE_MONITOR}"

hyprctl reload
