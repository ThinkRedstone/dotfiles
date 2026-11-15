function pull --description 'pull a file from this ssh server back to the client via kitty (default target /tmp)'
    if test (count $argv) -lt 1
        echo "pull: usage: pull SOURCE [TARGET]" >&2
        return 1
    end
    set -l source $argv[1]
    set -l target /tmp/
    if test (count $argv) -gt 1
        set target $argv[2]
    end
    # default --direction=download copies from the machine running the kitten
    # (this server) to the terminal's local machine (the client), over the TTY.
    # kitty pops a confirmation dialog on the client to approve the transfer.
    kitten transfer "$source" "$target"
end
