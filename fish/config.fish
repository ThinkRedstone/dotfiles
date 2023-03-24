if status is-interactive
    # Commands to run in interactive sessions can go here
    test -e venv/bin/activate.fish; and source venv/bin/activate.fish
end
