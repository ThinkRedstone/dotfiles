#!/bin/bash

THEME_DIR=$(dirname ${BASH_SOURCE})/../themes
THEME=$(realpath $THEME_DIR/${1}/)

if [[ ! -d $THEME ]]; then
  echo "No such theme: $1"
  exit 1
fi

# change theme
rm ~/.config/theme
ln -s "$THEME" ~/.config/theme

# reload wallpaper
hyprctl hyprpaper wallpaper ,~/.config/theme/wallpaper.png

# reload kitty conf
pkill -USR1 -f kitty

# reload waybar conf
pkill -USR2 -f waybar

# reload hyprctl (which is suppose to auto reload but doesn't)
hyprctl reload

pkill -USR1 -f fish
