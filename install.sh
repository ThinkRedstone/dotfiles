#!/bin/bash

link () {
  ln -s $(realpath $1) $2
}

config_link () {
  link $1 ~/.config
}

mkdir ~/.config

sudo pacman -S - < packages.txt

config_link fish
config_link xfce4

link .xinitrc ~/
