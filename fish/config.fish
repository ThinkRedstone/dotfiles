if status is-interactive
    # Commands to run in interactive sessions can go here
    test -e venv/bin/activate.fish; and source venv/bin/activate.fish
    test -e ~/.config/theme/fish_colors.fish; and source ~/.config/theme/fish_colors.fish
end
