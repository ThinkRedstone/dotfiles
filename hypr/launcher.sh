#!/bin/sh
monitor=$(hyprctl monitors | grep "focused: yes" -B 11 | head -n1 | cut -d" " -f 2)
dmenu-wl_run -sf '#ff0000' -sb '#111111' -m "$monitor"
