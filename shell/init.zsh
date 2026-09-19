# SparkleBIOS shell hook. Printed by `bios init zsh`.
# Add to the END of ~/.zshrc:
#   command -v bios >/dev/null 2>&1 && eval "$(bios init zsh)"
if [[ -o interactive ]] && command -v bios >/dev/null 2>&1; then
  bios boot
  export SPARKLEBIOS_BOOTED=1
fi
