function sshx --wraps ssh --description 'ssh, first rsyncing fish + vim config to the remote'
    set -l host $argv[-1]
    set -l ssh_opts
    if test (count $argv) -gt 1
        set ssh_opts $argv[1..-2]
    end
    # one rsync = one ssh connection for both trees:
    #   -R + the /./ marker set the remote-relative root (-> ~/.config/fish, ~/.vim)
    #   -L turns the ~/.config/fish symlink into real files on the remote
    #   excludes are anchored to the transfer root with a leading /
    rsync -azLR -h --info=progress2 -e "ssh $ssh_opts" \
        --exclude /.config/fish/fish_variables \
        --exclude /.vim/plugged --exclude /.vim/.git \
        ~/./.config/fish ~/./.vim ~/./.gitconfig "$host":

    kitten ssh $argv
end
