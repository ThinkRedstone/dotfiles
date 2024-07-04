function dotenv
  env (cat $argv[1] | sed 's/#.*$//;/^\s*$/d') fish
end
