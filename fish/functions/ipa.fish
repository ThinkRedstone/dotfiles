function ipa --wraps='ip --color --brief a' --description 'alias ipa ip --color --brief a'
  ip --color --brief a $argv; 
end
