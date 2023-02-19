function netflix --wraps='chromium --app="https://netflix.com"' --description 'alias netflix=chromium --app="https://netflix.com"'
  chromium --app="https://netflix.com" $argv; 
end
