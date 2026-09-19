# Rainbows and Unicorns: make everyday tools follow the terminal theme.
#
# Everything here uses the terminal's own sixteen ANSI colours by index, never a
# hard coded hex value, so it matches whichever theme variant you run and changes
# with it. Source this file from ~/.zshrc:
#
#   source /path/to/extras/tools/colours.zsh

# ls on macOS and the BSDs (ls -G): bold blue directories, cyan links, green executables.
export CLICOLOR=1
export LSCOLORS="ExGxFxdxCxegedabagacad"

# GNU ls, and anything else that reads LS_COLORS (eza, fd, tree, zsh completion).
export LS_COLORS="di=1;34:ln=36:so=35:pi=33:ex=32:bd=34;46:cd=34;43:su=30;41:sg=30;46:tw=30;42:ow=30;43:*.md=33:*.toml=33:*.lock=90"
zstyle ':completion:*' list-colors "${(s.:.)LS_COLORS}"

# bat: syntax highlighting drawn from the terminal palette.
export BAT_THEME="ansi"

# fzf: gold pointer like the cursor, orange marks, the rest from the palette.
export FZF_DEFAULT_OPTS="${FZF_DEFAULT_OPTS:+$FZF_DEFAULT_OPTS }--color=fg:-1,bg:-1,hl:3,fg+:15,bg+:0,hl+:11,info:6,prompt:2,pointer:3,marker:9,spinner:5,header:4,border:8"

# man pages and less: headings in yellow, emphasis in cyan.
export LESS_TERMCAP_md=$'\e[1;33m' LESS_TERMCAP_me=$'\e[0m'
export LESS_TERMCAP_us=$'\e[36m'   LESS_TERMCAP_ue=$'\e[0m'
export LESS_TERMCAP_so=$'\e[30;43m' LESS_TERMCAP_se=$'\e[0m'

# grep matches in orange (bright red carries the orange stripe in this theme).
export GREP_COLORS="mt=1;91"
