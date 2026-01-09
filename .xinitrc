#!/bin/bash
export QT_QPA_PLATFORMTHEME=qt5ct

xmonad --recompile

xbindkeys
xset -b
xsetroot -cursor_name left_ptr
setxkbmap -layout us,il -option 'grp:alt_caps_toggle'

( sleep 0.1 && ~/Scripts/startup_programs.sh ) &
( sleep 1 && polybar -c ~/.xmonad/polybar example) &

feh --bg-fill ~/.config/theme/wallpaper.png &

# xrandr --setprovideroutputsource modesetting NVIDIA-0
# xrandr --auto
exec xmonad

