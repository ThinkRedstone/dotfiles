#!/bin/bash

link () {
  ln -s $(realpath $1) $2
}

config_link () {
  link $1 ~/.config
}

mkdir ~/.config

sudo pacman -S - < packages.txt

git lfs install && git lfs pull

config_link fish
config_link xfce4

link bakuman.png ~/.config/wallpaper.png
link .xinitrc ~/
link .xbindkeysrc ~/
