#!/bin/bash

config() {
  if ( hyprctl monitors | grep "DELL U2723QE 2R0SWN3" ); then
    cat <<'EOF'
$left=desc:AU Optronics 0xD291
$middle=desc:Dell Inc. DELL U2723QE 2R0SWN3
$right=desc:Dell Inc. DELL U2723QE BS0SWN3
monitor=$left,preferred,-960x1140,2
monitor=$middle,preferred,0x0,auto
monitor=$right,preferred,2560x-580,auto,transform,1
EOF
  else
    cat <<'EOF'
$middle=desc:AU Optronics 0xD291
monitor=$middle,preferred,0x0,1
EOF
  fi
}

config > ~/.monitors.conf

