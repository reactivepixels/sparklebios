//! Embedded hook text, and filling in what the flavour says.
//!
//! The hooks are templates. `bios init <shell>` writes the current flavour's lines straight into
//! the text it prints, so the shell holds the strings itself and never spawns `bios` to speak.

pub const ZSH_HOOK: &str = include_str!("../shell/init.zsh");
pub const BASH_HOOK: &str = include_str!("../shell/init.bash");
pub const FISH_HOOK: &str = include_str!("../shell/init.fish");

/// The shells `bios init` prints a hook for. Which template to fill in, how a prompt must be told
/// to ignore an escape, and how a string is quoted all differ between them and all have to agree,
/// so they are decided in one place rather than passed around separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Zsh,
    Bash,
    Fish,
}

impl Shell {
    pub fn template(self) -> &'static str {
        match self {
            Shell::Zsh => ZSH_HOOK,
            Shell::Bash => BASH_HOOK,
            Shell::Fish => FISH_HOOK,
        }
    }

    pub fn wrap(self) -> Wrap {
        match self {
            Shell::Zsh => Wrap::Zsh,
            Shell::Bash => Wrap::Bash,
            // fish measures the prompt itself, so nothing needs marking.
            Shell::Fish => Wrap::None,
        }
    }

    /// `s` as a single quoted word this shell reads back exactly, whatever is in it.
    ///
    /// The two families disagree about the backslash. In sh and zsh nothing inside single quotes
    /// is special except the quote itself, so a quote is closed, escaped and reopened. In fish
    /// both `\` and `'` stay special inside single quotes, so both are escaped. Getting this
    /// wrong is not a cosmetic bug: the kitty image ends in `ESC \`, so a backslash left alone in
    /// fish escapes the closing quote and the rest of the hook is swallowed by an unclosed string.
    fn quote(self, s: &str) -> String {
        match self {
            Shell::Fish => format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'")),
            _ => format!("'{}'", s.replace('\'', "'\\''")),
        }
    }
}

/// Which shell's escaping a mascot placement needs, so the prompt's width accounting stays right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wrap {
    /// zsh: `%{ %}` around anything that prints nothing.
    Zsh,
    /// bash: `\[ \]`.
    Bash,
    /// No wrapping, for fish and for starship, which work it out themselves.
    None,
}

/// The mascot's two halves: the bytes that send the image once, and the short placement that a
/// prompt repeats. Both empty where the terminal cannot draw an image. `transmit` is the raw
/// escape, not a command: the hook holds it in a variable and prints it, so the placeholder is
/// never in command position and the template stays a parseable shell script.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mascot {
    pub transmit: String,
    pub placement: String,
}

impl Mascot {
    /// Nothing at all: the prompt is left exactly as it was.
    pub fn none() -> Mascot {
        Mascot::default()
    }

    /// A kitty placement. The image goes once, with an id derived from the flavour, and each
    /// prompt then costs only the placement, which is a few dozen bytes.
    pub fn kitty(png: &[u8], id: u32, rows: u16, wrap: Wrap) -> Mascot {
        let cols = rows * 2;
        let transmit = crate::sprite::kitty_transmit(png, id);
        let placement = wrapped(
            &format!("\x1b_Ga=p,i={id},c={cols},r={rows},q=2\x1b\\"),
            cols,
            wrap,
        );
        Mascot {
            transmit,
            placement,
        }
    }

    /// An iTerm2 inline image. There is nothing to send once here: the protocol has no stored
    /// image and no placement, so the variable holds the picture itself and the shell reprints it
    /// every prompt. That is about 3KB a prompt, which is a lot more than kitty's placement, but
    /// it is still only a printf of a string the shell already holds, and nothing is spawned.
    pub fn iterm(png: &[u8], rows: u16, wrap: Wrap) -> Mascot {
        let cols = rows * 2;
        Mascot {
            transmit: String::new(),
            placement: wrapped(&crate::sprite::iterm_image(png, cols, rows), cols, wrap),
        }
    }

    /// Adds the boot streak after the mascot, when there is a streak worth mentioning. One day is
    /// not a streak, so it starts at two.
    ///
    /// This is read once, when the hook is generated, so it is fixed for that shell's lifetime. A
    /// tab left open across midnight will show yesterday's number until it is reopened. That is
    /// the price of never spawning anything per prompt, and it is the right way round.
    pub fn with_streak(mut self, days: u32) -> Mascot {
        if days >= 2 {
            // The trailing space is the gap to whatever prompt follows, so the variable can be
            // put straight in front of one without the user adding spacing of their own.
            self.placement.push_str(&format!("{days}d "));
        }
        self
    }
}

/// Wraps an escape that prints nothing so the shell does not count it toward the prompt's width,
/// then adds the spaces the image actually covers, which the shell must count.
fn wrapped(escape: &str, cols: u16, wrap: Wrap) -> String {
    let spaces = " ".repeat(cols as usize);
    match wrap {
        Wrap::Zsh => format!("%{{{escape}%}}{spaces}"),
        Wrap::Bash => format!("\\[{escape}\\]{spaces}"),
        Wrap::None => format!("{escape}{spaces}"),
    }
}

fn bit(on: bool) -> &'static str {
    if on {
        "1"
    } else {
        "0"
    }
}

/// A flavour's line as one shell word, with `{took}` spliced out as a reference to the variable
/// the hook has just worked out.
///
/// Quoting each literal piece separately and leaving only the variable reference outside the
/// quotes is what keeps a flavour's words words. Written into a double quoted string instead, a
/// line holding a `$`, a backtick or a quote would be read as shell by every terminal that
/// sourced the hook, and a user flavour is just a file on disk. Adjacent quoted pieces are one
/// word in all three shells, so the whole line still arrives as a single argument.
fn word(shell: Shell, text: &str, took: &str) -> String {
    let mut out = String::new();
    for (i, part) in text.split("{took}").enumerate() {
        if i > 0 {
            out.push_str(took);
        }
        if !part.is_empty() {
            out.push_str(&shell.quote(part));
        }
    }
    if out.is_empty() {
        out.push_str("''");
    }
    out
}

/// The hook text for a shell, with the flavour's lines and the user's settings written into it.
/// Turbo off and no sounds directory: the caller has no ULTRA state to give (or, at any level
/// other than `Ultra`, none that would ever be read). See `render_hook` for the full version
/// `bios init` actually uses, which takes both.
pub fn render(
    shell: Shell,
    config: &crate::config::Config,
    flavour: Option<&crate::flavour::Flavour>,
    mascot: Mascot,
) -> String {
    render_hook(shell, config, flavour, mascot, false, "")
}

/// `render`, plus the two pieces of state only the ULTRA sprinkles level needs.
///
/// `turbo` is the Turbo flag's value at the moment the hook is generated (see `state::State`):
/// like the mascot and the boot streak, it is read once and baked in, fixed for that shell's
/// lifetime. `sounds_dir` is where the synthesised ULTRA sounds live; it is only ever read from
/// when the effective sprinkles level is `Ultra` (see `ultra_block`), so a caller at any other
/// level may pass anything, including a path that does not exist.
pub fn render_hook(
    shell: Shell,
    config: &crate::config::Config,
    flavour: Option<&crate::flavour::Flavour>,
    mascot: Mascot,
    turbo: bool,
    sounds_dir: &str,
) -> String {
    let quote = |s: &str| shell.quote(s);
    let say = |event: &str| -> String {
        flavour
            .and_then(|f| f.presence.get(event).cloned())
            .unwrap_or_default()
    };

    // `{took}` is the one slot the shell fills in itself, from the variable it just computed.
    let took = match shell {
        Shell::Fish => "\"$took\"",
        _ => "\"${took}\"",
    };
    let done = word(shell, &say("done"), took);
    let failed = word(shell, &say("failed"), took);

    let rendered = shell
        .template()
        .replace("{{PRESENCE}}", bit(config.presence && flavour.is_some()))
        .replace("{{TITLE}}", bit(config.title && flavour.is_some()))
        .replace("{{AFTER}}", &config.presence_after.to_string())
        .replace("{{TITLE_NAME}}", &quote(&say("title")))
        .replace("{{DONE}}", &done)
        .replace("{{FAILED}}", &failed)
        .replace(
            "{{GOODBYE_TRAP}}",
            &quote(&format!("printf '%s\\n' {}", quote(&say("goodbye")))),
        )
        .replace("{{GOODBYE}}", &quote(&say("goodbye")))
        .replace("{{NOT_FOUND}}", &quote(&say("not_found")))
        .replace("{{MASCOT}}", &quote(&mascot.placement))
        .replace("{{MASCOT_IMAGE}}", &quote(&mascot.transmit));
    splice_ultra_line(
        rendered,
        shell,
        &ultra_block(shell, config, flavour, turbo, sounds_dir),
    )
}

/// Every template has exactly one `eval {{ULTRA}}` line (see `shell/init.*`): a plain `eval` of
/// a bareword is valid raw syntax in all three shells even before this token is filled in, which
/// is what keeps `hook_is_valid_zsh` and its bash and fish equivalents (which parse the template
/// files themselves, unrendered) passing. Below `Ultra`, `ultra` is empty and this removes that
/// whole line, indentation and trailing newline included, rather than leaving an `eval ''`
/// behind: a hook at any other level must be byte for byte what it was before this feature
/// existed, and a person who has not asked for ultra should not find a line explaining a seam
/// they do not have. At `Ultra`, the line becomes `eval` followed by the block quoted as one
/// shell word, the same deferred-execution trick `{{GOODBYE_TRAP}}` already uses, so the block
/// runs exactly as if it had been inlined.
fn splice_ultra_line(rendered: String, shell: Shell, ultra: &str) -> String {
    const MARKER: &str = "eval {{ULTRA}}";
    let Some(marker_start) = rendered.find(MARKER) else {
        return rendered;
    };
    let line_start = rendered[..marker_start].rfind('\n').map_or(0, |i| i + 1);
    let after_marker = marker_start + MARKER.len();
    let line_end = rendered[after_marker..]
        .find('\n')
        .map_or(rendered.len(), |i| after_marker + i + 1);

    let mut out = String::with_capacity(rendered.len());
    out.push_str(&rendered[..line_start]);
    if !ultra.is_empty() {
        out.push_str(&rendered[line_start..marker_start]);
        out.push_str("eval ");
        out.push_str(&shell.quote(ultra));
        out.push('\n');
    }
    out.push_str(&rendered[line_end..]);
    out
}

/// Everything the ULTRA sprinkles block needs baked into the hook: whether afplay-style sound
/// is muted (`ultra_volume` baked to 0), the flavour's jingle name, and where the synthesised
/// sounds live. Empty (`""`) when the effective sprinkles level is not `Ultra`, so `render`'s
/// `{{ULTRA}}` placeholder disappears entirely and the rendered hook is byte for byte what it
/// is at `off`, `light` and `full` (see `an_ultra_block_is_empty_unless_the_level_is_ultra`).
fn ultra_block(
    shell: Shell,
    config: &crate::config::Config,
    flavour: Option<&crate::flavour::Flavour>,
    turbo: bool,
    sounds_dir: &str,
) -> String {
    if config.sprinkles != crate::sprinkles::Level::Ultra {
        return String::new();
    }
    let sound_on = if config.ultra_volume > 0.0 { "1" } else { "0" };
    let turbo_bit = bit(turbo);
    let jingle = flavour.map(|f| format!("jingle_{}", f.id));
    let vars = UltraVars {
        sounds_dir: shell.quote(sounds_dir),
        chance: config.ultra_chance.to_string(),
        volume: config.ultra_volume.to_string(),
        sound_on: sound_on.to_string(),
        turbo_bit: turbo_bit.to_string(),
        after: config.presence_after.to_string(),
        presence: bit(config.presence && flavour.is_some()).to_string(),
        jingle_line: jingle.map(|name| format!("_sparklebios_ultra_sound {}", shell.quote(&name))),
    };
    match shell {
        Shell::Zsh => ultra_block_zsh(&vars),
        Shell::Bash => ultra_block_bash(&vars),
        Shell::Fish => ultra_block_fish(&vars),
    }
}

/// The pieces of the ULTRA block that differ per shell start from the same values; this is just
/// somewhere to hold them so the three builders below take one argument instead of seven.
struct UltraVars {
    sounds_dir: String,
    chance: String,
    volume: String,
    sound_on: String,
    turbo_bit: String,
    after: String,
    presence: String,
    jingle_line: Option<String>,
}

/// Fills the `@@TOKEN@@` placeholders a shell's ULTRA block template uses. Kept separate from
/// `render`'s own `{{TOKEN}}` substitutions (different bracket style on purpose) so the two
/// passes can never collide: this one runs first, while the block is still a standalone string,
/// and its result is then dropped whole into `{{ULTRA}}` by `render`.
fn fill_ultra_vars(template: &str, vars: &UltraVars) -> String {
    let jingle_line = vars.jingle_line.clone().unwrap_or_default();
    template
        .replace("@@SOUNDS@@", &vars.sounds_dir)
        .replace("@@CHANCE@@", &vars.chance)
        .replace("@@VOLUME@@", &vars.volume)
        .replace("@@SOUND_ON@@", &vars.sound_on)
        .replace("@@TURBO@@", &vars.turbo_bit)
        .replace("@@AFTER@@", &vars.after)
        .replace("@@PRESENCE@@", &vars.presence)
        .replace("@@JINGLE_LINE@@", &jingle_line)
}

/// The ULTRA block for zsh: moments 1 through 9 in full, since zsh's `zselect` gives it a true
/// builtin wait with no external `sleep` and `add-zsh-hook` lets several hooks share one event,
/// so the block can sit on its own, ahead of the title and presence blocks, and every moment in
/// the plan is reachable. Registered first (see the placement in `shell/init.zsh`) so its own
/// `precmd` sees the real `$?` before title's or presence's own hooks have a chance to run a
/// command of their own and overwrite it.
fn ultra_block_zsh(vars: &UltraVars) -> String {
    fill_ultra_vars(
        r##"if [[ -z "${SSH_CONNECTION:-}" ]]; then
    typeset -g _sparklebios_ultra_sounds=@@SOUNDS@@
    typeset -g _sparklebios_ultra_chance=@@CHANCE@@
    typeset -g _sparklebios_ultra_volume=@@VOLUME@@
    typeset -gi _sparklebios_ultra_sound_on=@@SOUND_ON@@
    typeset -gi _sparklebios_ultra_turbo=@@TURBO@@
    typeset -gx SPARKLEBIOS_STATUS=""
    typeset -gF _sparklebios_ultra_t0=0
    typeset -g _sparklebios_ultra_cmd="" _sparklebios_ultra_last_failed=""
    zmodload zsh/datetime zsh/zselect 2>/dev/null

    if (( $+commands[afplay] )); then
      _sparklebios_ultra_play() { exec afplay -v "$_sparklebios_ultra_volume" "$1" }
    elif (( $+commands[pw-play] )); then
      _sparklebios_ultra_play() { exec pw-play "$1" }
    elif (( $+commands[paplay] )); then
      _sparklebios_ultra_play() { exec paplay "$1" }
    elif (( $+commands[aplay] )); then
      _sparklebios_ultra_play() { exec aplay -q "$1" }
    else
      _sparklebios_ultra_play() { : }
    fi

    _sparklebios_ultra_sound() {
      (( _sparklebios_ultra_sound_on )) || return 0
      local f="$_sparklebios_ultra_sounds/$1.wav"
      [[ -r $f ]] || return 0
      _sparklebios_ultra_play "$f" &>/dev/null &!
    }

    _sparklebios_ultra_maybe() { (( RANDOM % 100 < ${1:-$_sparklebios_ultra_chance} )) }

    _sparklebios_ultra_everyday() {
      case $1 in
        (git\ push*|ssh\ *|scp\ *|rsync\ *) _sparklebios_ultra_sound modem ;;
        (git\ pull*|git\ fetch*|curl\ *|wget\ *|npm\ i*|pnpm\ i*|cargo\ build*|make*)
          _sparklebios_ultra_maybe 50 && _sparklebios_ultra_sound modem ;;
        (ls|ls\ *|la|la\ *|ll|ll\ *|tree|tree\ *)
          _sparklebios_ultra_maybe 10 && _sparklebios_ultra_sound floppy_seek ;;
        (git\ status*|git\ log*|git\ diff*)
          _sparklebios_ultra_maybe 30 && _sparklebios_ultra_sound floppy_seek ;;
        (rm\ *|mv\ *|cp\ *|mkdir\ *|touch\ *)
          _sparklebios_ultra_maybe 15 && _sparklebios_ultra_sound hdd_spindown ;;
      esac
      return 0
    }

    _sparklebios_ultra_preexec() {
      _sparklebios_ultra_t0=$EPOCHREALTIME
      _sparklebios_ultra_cmd=$1
      _sparklebios_ultra_everyday "$1"
      (( _sparklebios_ultra_sound_on )) || return 0
      ( zselect -t 200; _sparklebios_ultra_play "$_sparklebios_ultra_sounds/hdd_chatter.wav" ) &>/dev/null &!
    }

    _sparklebios_ultra_rain() {
      [[ -t 1 ]] || return 0
      [[ -n "${NO_COLOR:-}" ]] && return 0
      local -F secs=${1:-1.5}
      local cols=$COLUMNS rows=$LINES i x y c
      local -a glyphs=('*' '+' '.' 'o' '•' '✦' '✧') colours=(1 3 2 6 4 5)
      local -a drops
      local out
      print -n '\e[?1049h\e[?25l\e[2J'
      for (( i = 1; i <= cols / 3; i++ )); do drops+=( "$(( RANDOM % cols + 1 )):$(( RANDOM % rows + 1 ))" ); done
      local -F t0=$EPOCHREALTIME
      while (( EPOCHREALTIME - t0 < secs )); do
        out=""
        for (( i = 1; i <= $#drops; i++ )); do
          x=${drops[i]%%:*}; y=${drops[i]#*:}
          (( y > 1 )) && out+="\e[$(( y - 1 ));${x}H "
          c=${colours[$(( RANDOM % 6 + 1 ))]}
          out+="\e[${y};${x}H\e[9${c}m${glyphs[$(( RANDOM % $#glyphs + 1 ))]}\e[0m"
          (( y += 1 + RANDOM % 2 ))
          if (( y > rows )); then y=1; x=$(( RANDOM % cols + 1 )); fi
          drops[i]="$x:$y"
        done
        print -n -- "$out"
        zselect -t 4
      done
      print -n '\e[2J\e[?25h\e[?1049l'
    }

    _sparklebios_ultra_precmd() {
      local ultra_status=$?
      if (( ultra_status != 0 && ultra_status != 130 )); then
        SPARKLEBIOS_STATUS=$(printf 'ERR 0x%02X' $ultra_status)
      else
        SPARKLEBIOS_STATUS=""
      fi
      (( _sparklebios_ultra_turbo )) && SPARKLEBIOS_STATUS="$SPARKLEBIOS_STATUS 66MHz"
      [[ -z $_sparklebios_ultra_cmd ]] && return
      local -F ultra_took=$(( EPOCHREALTIME - _sparklebios_ultra_t0 ))
      if (( ultra_took >= @@AFTER@@ )) && [[ @@PRESENCE@@ == 1 ]]; then
        (( ultra_status == 0 )) && _sparklebios_ultra_sound post_ok || _sparklebios_ultra_sound post_fail
      elif (( ultra_took >= 2 )); then
        _sparklebios_ultra_sound hdd_spindown
      fi
      if (( ultra_status == 0 )); then
        case $_sparklebios_ultra_cmd in
          (git\ commit*)
            if [[ -n "${NO_COLOR:-}" ]]; then
              printf '%s\n' 'Saving to CMOS... done.'
            else
              printf '\e[36m%s\e[0m\n' 'Saving to CMOS... done.'
            fi ;;
          (git\ push*) _sparklebios_ultra_rain 1.2 ;;
        esac
        if [[ -n $_sparklebios_ultra_last_failed ]] && (( ultra_took >= 2 )); then
          _sparklebios_ultra_rain 1.6
        fi
        _sparklebios_ultra_last_failed=""
      else
        (( ultra_status != 130 )) && _sparklebios_ultra_last_failed=1
      fi
      _sparklebios_ultra_cmd=""
    }

    _sparklebios_ultra_chpwd() {
      if [[ -d .git ]]; then
        _sparklebios_ultra_maybe 60 && _sparklebios_ultra_sound floppy_seek
      else
        _sparklebios_ultra_maybe 25 && _sparklebios_ultra_sound floppy_seek
      fi
      _sparklebios_ultra_maybe || return 0
      [[ -n "${NO_COLOR:-}" ]] && return 0
      local -a entries; entries=( *(DN) )
      local n=$#entries dir=${PWD:t} line newest age branch changes
      case $(( RANDOM % 4 )) in
        (0) line="Seek complete. $n entries in $dir." ;;
        (1)
          newest=( *(DNom[1]) )
          if [[ -n $newest ]]; then
            local -a mtimes
            zstat -A mtimes +mtime -- "$newest" 2>/dev/null
            if [[ -n $mtimes ]]; then
              age=$(( (EPOCHSECONDS - mtimes[1]) / 60 ))
              if (( age < 60 )); then
                line="$dir: last write ${age}m ago. Still warm."
              elif (( age < 1440 )); then
                line="$dir: last write $(( age / 60 ))h ago."
              else
                line="$dir: last write $(( age / 1440 )) days ago. Dusty."
              fi
            fi
          fi ;;
        (2)
          if [[ -d .git ]]; then
            branch=$(git --no-optional-locks symbolic-ref --short HEAD 2>/dev/null)
            changes=$(git --no-optional-locks status --porcelain 2>/dev/null | wc -l | tr -d ' ')
            [[ -n $branch ]] && line="Mounted $dir on $branch. $changes uncommitted."
          fi ;;
        (3) (( n == 0 )) && line="$dir is empty. The BIOS respects that." ;;
      esac
      [[ -n $line ]] && printf '\e[90m%s\e[0m\n' "$line"
    }

    _sparklebios_ultra_exit() {
      _sparklebios_ultra_sound power_off
      [[ -t 1 ]] || return 0
      [[ -n "${NO_COLOR:-}" ]] && return 0
      local mid=$(( LINES / 2 )) w
      print -n '\e[?25l\e[2J'
      for w in $COLUMNS $(( COLUMNS * 2 / 3 )) $(( COLUMNS / 3 )) $(( COLUMNS / 8 )) 1; do
        print -n "\e[${mid};1H\e[2K\e[${mid};$(( (COLUMNS - w) / 2 + 1 ))H\e[97m${(l:$w::━:)}\e[0m"
        zselect -t 5
      done
      zselect -t 8
      print -n '\e[2J\e[H\e[?25h'
    }

    autoload -Uz add-zsh-hook 2>/dev/null
    if (( $+functions[add-zsh-hook] )); then
      add-zsh-hook preexec _sparklebios_ultra_preexec
      add-zsh-hook precmd _sparklebios_ultra_precmd
      add-zsh-hook chpwd _sparklebios_ultra_chpwd
      add-zsh-hook zshexit _sparklebios_ultra_exit
    fi

    @@JINGLE_LINE@@
  fi
"##,
        vars,
    )
}

/// The ULTRA block for bash. Registered after the title and presence blocks in
/// `shell/init.bash`, on purpose: bash's `PROMPT_COMMAND` is a plain string that each block
/// prepends itself to, so the block added *last* ends up running *first*, which is what lets
/// this one read the command's real `$?` before title's own `printf` or presence's own precmd
/// touches it. bash allows only one `DEBUG` trap and one `EXIT` trap at a time, so rather than
/// clobbering presence's, this captures whatever is already trapped (with `trap -p`) and chains
/// it in, unwrapping its own previous chain first so a second `eval "$(bios init bash)"` does
/// not nest a copy of itself into the trap string every time.
///
/// bash has no builtin wait, so unlike zsh this cannot start `hdd_chatter` two seconds into a
/// still-running command without spawning `sleep` for the wait itself, which would spawn on
/// every long command rather than only at the detached player; that half of moment 2 is left
/// out here. The rest of moment 2 (the sounds once a command has already finished) does not
/// need a mid-command timer and is included.
fn ultra_block_bash(vars: &UltraVars) -> String {
    fill_ultra_vars(
        r##"if [[ -z "${SSH_CONNECTION:-}" ]]; then
    _sparklebios_ultra_sounds=@@SOUNDS@@
    _sparklebios_ultra_chance=@@CHANCE@@
    _sparklebios_ultra_volume=@@VOLUME@@
    _sparklebios_ultra_sound_on=@@SOUND_ON@@
    _sparklebios_ultra_turbo=@@TURBO@@
    export SPARKLEBIOS_STATUS=""
    _sparklebios_ultra_cmd=""
    _sparklebios_ultra_last_failed=""
    _sparklebios_ultra_prev_pwd="$PWD"

    if command -v afplay >/dev/null 2>&1; then
      _sparklebios_ultra_play() { exec afplay -v "$_sparklebios_ultra_volume" "$1"; }
    elif command -v pw-play >/dev/null 2>&1; then
      _sparklebios_ultra_play() { exec pw-play "$1"; }
    elif command -v paplay >/dev/null 2>&1; then
      _sparklebios_ultra_play() { exec paplay "$1"; }
    elif command -v aplay >/dev/null 2>&1; then
      _sparklebios_ultra_play() { exec aplay -q "$1"; }
    else
      _sparklebios_ultra_play() { :; }
    fi

    _sparklebios_ultra_sound() {
      [[ "$_sparklebios_ultra_sound_on" == 1 ]] || return 0
      local f="$_sparklebios_ultra_sounds/$1.wav"
      [[ -r "$f" ]] || return 0
      _sparklebios_ultra_play "$f" >/dev/null 2>&1 &
      disown 2>/dev/null
    }

    _sparklebios_ultra_maybe() {
      local chance=${1:-$_sparklebios_ultra_chance}
      (( RANDOM % 100 < chance ))
    }

    _sparklebios_ultra_everyday() {
      case $1 in
        git\ push*|ssh\ *|scp\ *|rsync\ *) _sparklebios_ultra_sound modem ;;
        git\ pull*|git\ fetch*|curl\ *|wget\ *|npm\ i*|pnpm\ i*|cargo\ build*|make*)
          _sparklebios_ultra_maybe 50 && _sparklebios_ultra_sound modem ;;
        ls|ls\ *|la|la\ *|ll|ll\ *|tree|tree\ *)
          _sparklebios_ultra_maybe 10 && _sparklebios_ultra_sound floppy_seek ;;
        git\ status*|git\ log*|git\ diff*)
          _sparklebios_ultra_maybe 30 && _sparklebios_ultra_sound floppy_seek ;;
        rm\ *|mv\ *|cp\ *|mkdir\ *|touch\ *)
          _sparklebios_ultra_maybe 15 && _sparklebios_ultra_sound hdd_spindown ;;
      esac
      return 0
    }

    _sparklebios_ultra_rain() {
      [[ -t 1 ]] || return 0
      [[ -n "${NO_COLOR:-}" ]] && return 0
      local secs=${1:-1} cols=${COLUMNS:-80} rows=${LINES:-24} i x y c glyph out start
      local glyphs=('*' '+' '.' 'o' '•' '✦' '✧') colours=(1 3 2 6 4 5)
      local -a dropx dropy
      local ndrops=$(( cols / 3 ))
      (( ndrops < 1 )) && ndrops=1
      printf '\e[?1049h\e[?25l\e[2J'
      for (( i = 0; i < ndrops; i++ )); do
        dropx[i]=$(( RANDOM % cols + 1 )); dropy[i]=$(( RANDOM % rows + 1 ))
      done
      start=$SECONDS
      while (( SECONDS - start < secs )); do
        out=""
        for (( i = 0; i < ndrops; i++ )); do
          x=${dropx[i]}; y=${dropy[i]}
          (( y > 1 )) && out+="\e[$(( y - 1 ));${x}H "
          c=${colours[RANDOM % 6]}
          glyph=${glyphs[RANDOM % 7]}
          out+="\e[${y};${x}H\e[9${c}m${glyph}\e[0m"
          y=$(( y + 1 + RANDOM % 2 ))
          if (( y > rows )); then y=1; x=$(( RANDOM % cols + 1 )); fi
          dropx[i]=$x; dropy[i]=$y
        done
        printf '%b' "$out"
        sleep 0.04
      done
      printf '\e[2J\e[?25h\e[?1049l'
    }

    _sparklebios_ultra_chpwd() {
      if [[ -d .git ]]; then
        _sparklebios_ultra_maybe 60 && _sparklebios_ultra_sound floppy_seek
      else
        _sparklebios_ultra_maybe 25 && _sparklebios_ultra_sound floppy_seek
      fi
      _sparklebios_ultra_maybe || return 0
      [[ -n "${NO_COLOR:-}" ]] && return 0
      shopt -s dotglob nullglob
      local -a entries=(*)
      shopt -u dotglob nullglob
      local n=${#entries[@]} dir=${PWD##*/} line="" newest="" newest_mtime=0 age branch changes mtime f
      [[ -z "$dir" ]] && dir=/
      case $(( RANDOM % 4 )) in
        0) line="Seek complete. $n entries in $dir." ;;
        1)
          for f in "${entries[@]}"; do
            mtime=$(stat -f %m -- "$f" 2>/dev/null || stat -c %Y -- "$f" 2>/dev/null)
            [[ -n "$mtime" ]] || continue
            if (( mtime > newest_mtime )); then newest_mtime=$mtime; newest=$f; fi
          done
          if [[ -n "$newest" ]]; then
            age=$(( ($(date +%s) - newest_mtime) / 60 ))
            if (( age < 60 )); then
              line="$dir: last write ${age}m ago. Still warm."
            elif (( age < 1440 )); then
              line="$dir: last write $(( age / 60 ))h ago."
            else
              line="$dir: last write $(( age / 1440 )) days ago. Dusty."
            fi
          fi ;;
        2)
          if [[ -d .git ]]; then
            branch=$(git --no-optional-locks symbolic-ref --short HEAD 2>/dev/null)
            changes=$(git --no-optional-locks status --porcelain 2>/dev/null | wc -l | tr -d ' ')
            [[ -n "$branch" ]] && line="Mounted $dir on $branch. $changes uncommitted."
          fi ;;
        3) (( n == 0 )) && line="$dir is empty. The BIOS respects that." ;;
      esac
      [[ -n "$line" ]] && printf '\e[90m%s\e[0m\n' "$line"
    }

    _sparklebios_ultra_precmd() {
      local ultra_status=$?
      if (( ultra_status != 0 && ultra_status != 130 )); then
        SPARKLEBIOS_STATUS=$(printf 'ERR 0x%02X' "$ultra_status")
      else
        SPARKLEBIOS_STATUS=""
      fi
      [[ "$_sparklebios_ultra_turbo" == 1 ]] && SPARKLEBIOS_STATUS="$SPARKLEBIOS_STATUS 66MHz"

      if [[ -n "$_sparklebios_ultra_cmd" ]]; then
        local ultra_took=$(( SECONDS - _sparklebios_ultra_started ))
        if (( ultra_took >= @@AFTER@@ )) && [[ @@PRESENCE@@ == 1 ]]; then
          if (( ultra_status == 0 )); then _sparklebios_ultra_sound post_ok; else _sparklebios_ultra_sound post_fail; fi
        elif (( ultra_took >= 2 )); then
          _sparklebios_ultra_sound hdd_spindown
        fi
        if (( ultra_status == 0 )); then
          case $_sparklebios_ultra_cmd in
            git\ commit*)
              if [[ -n "${NO_COLOR:-}" ]]; then
                printf '%s\n' 'Saving to CMOS... done.'
              else
                printf '\e[36m%s\e[0m\n' 'Saving to CMOS... done.'
              fi ;;
            git\ push*) _sparklebios_ultra_rain 1.2 ;;
          esac
          if [[ -n "$_sparklebios_ultra_last_failed" ]] && (( ultra_took >= 2 )); then
            _sparklebios_ultra_rain 1.6
          fi
          _sparklebios_ultra_last_failed=""
        else
          [[ "$ultra_status" != 130 ]] && _sparklebios_ultra_last_failed=1
        fi
        _sparklebios_ultra_cmd=""
        unset _sparklebios_ultra_started
      fi

      if [[ "$PWD" != "$_sparklebios_ultra_prev_pwd" ]]; then
        _sparklebios_ultra_prev_pwd="$PWD"
        _sparklebios_ultra_chpwd
      fi
    }

    _sparklebios_ultra_prior_debug="$(trap -p DEBUG)"
    _sparklebios_ultra_prior_debug="${_sparklebios_ultra_prior_debug#trap -- }"
    _sparklebios_ultra_prior_debug="${_sparklebios_ultra_prior_debug% DEBUG}"
    if [[ "$_sparklebios_ultra_prior_debug" == *"; _sparklebios_ultra_preexec" ]]; then
      _sparklebios_ultra_prior_debug="${_sparklebios_ultra_prior_debug%; _sparklebios_ultra_preexec}"
      _sparklebios_ultra_prior_debug="${_sparklebios_ultra_prior_debug#eval }"
    fi
    _sparklebios_ultra_preexec() {
      [[ -n "${COMP_LINE:-}" ]] && return
      # Not just our own precmd: PROMPT_COMMAND is title's and presence's own entries too, each
      # a simple command in its own right, so each of those also trips DEBUG once as part of
      # the very prompt-command run this preexec is trying to see past. Recognised by the
      # prefix every one of this project's own hook functions shares, rather than by name, so a
      # later entry chained in the same way does not have to be taught here separately.
      [[ "$BASH_COMMAND" == _sparklebios_* ]] && return
      [[ -z "${_sparklebios_ultra_started:-}" ]] || return
      _sparklebios_ultra_started=$SECONDS
      _sparklebios_ultra_cmd="$BASH_COMMAND"
      _sparklebios_ultra_everyday "$BASH_COMMAND"
    }
    trap "eval $_sparklebios_ultra_prior_debug; _sparklebios_ultra_preexec" DEBUG

    if [[ "${PROMPT_COMMAND:-}" != *_sparklebios_ultra_precmd* ]]; then
      PROMPT_COMMAND="_sparklebios_ultra_precmd${PROMPT_COMMAND:+; $PROMPT_COMMAND}"
    fi

    _sparklebios_ultra_exit() {
      _sparklebios_ultra_sound power_off
      [[ -t 1 ]] || return 0
      [[ -n "${NO_COLOR:-}" ]] && return 0
      local mid=$(( ${LINES:-24} / 2 )) cols=${COLUMNS:-80} w bar i
      printf '\e[?25l\e[2J'
      for w in "$cols" $(( cols * 2 / 3 )) $(( cols / 3 )) $(( cols / 8 )) 1; do
        # No `tr`: BSD tr (macOS) has no `\xHH` hex escape, so building the bar a character at a
        # time is the one way that reads the same width on both BSD and GNU userlands.
        bar=""
        for (( i = 0; i < w; i++ )); do bar+="━"; done
        printf '\e[%d;1H\e[2K\e[%d;%dH\e[97m%s\e[0m' "$mid" "$mid" $(( (cols - w) / 2 + 1 )) "$bar"
        sleep 0.05
      done
      sleep 0.08
      printf '\e[2J\e[H\e[?25h'
    }
    _sparklebios_ultra_prior_exit="$(trap -p EXIT)"
    _sparklebios_ultra_prior_exit="${_sparklebios_ultra_prior_exit#trap -- }"
    _sparklebios_ultra_prior_exit="${_sparklebios_ultra_prior_exit% EXIT}"
    if [[ "$_sparklebios_ultra_prior_exit" == "_sparklebios_ultra_exit; eval "* ]]; then
      _sparklebios_ultra_prior_exit="${_sparklebios_ultra_prior_exit#_sparklebios_ultra_exit; eval }"
    fi
    trap "_sparklebios_ultra_exit; eval $_sparklebios_ultra_prior_exit" EXIT

    @@JINGLE_LINE@@
  fi
"##,
        vars,
    )
}

/// The ULTRA block for fish. Registered ahead of the title and presence blocks in
/// `shell/init.fish`, the same reason as zsh: fish calls multiple handlers of the same event in
/// the order they were defined, so defining this one first is what lets its own `fish_prompt`
/// handler read `$status` before title's or presence's own handler runs a command of its own.
/// fish's events (`--on-event`, `--on-variable`) allow any number of independent handlers, so
/// unlike bash there is no trap to chain.
///
/// fish has no builtin wait either, so the same half of moment 2 (starting `hdd_chatter` while
/// a command is still running) is left out here for the same reason as bash.
fn ultra_block_fish(vars: &UltraVars) -> String {
    fill_ultra_vars(
        r##"if not set -q SSH_CONNECTION
        set -g _sparklebios_ultra_sounds @@SOUNDS@@
        set -g _sparklebios_ultra_chance @@CHANCE@@
        set -g _sparklebios_ultra_volume @@VOLUME@@
        set -g _sparklebios_ultra_sound_on @@SOUND_ON@@
        set -g _sparklebios_ultra_turbo @@TURBO@@
        set -gx SPARKLEBIOS_STATUS ""
        set -g _sparklebios_ultra_cmd ""
        set -g _sparklebios_ultra_last_failed 0
        set -g _sparklebios_ultra_started 0

        if command -v afplay >/dev/null 2>&1
            set -g _sparklebios_ultra_player afplay -v $_sparklebios_ultra_volume
        else if command -v pw-play >/dev/null 2>&1
            set -g _sparklebios_ultra_player pw-play
        else if command -v paplay >/dev/null 2>&1
            set -g _sparklebios_ultra_player paplay
        else if command -v aplay >/dev/null 2>&1
            set -g _sparklebios_ultra_player aplay -q
        else
            set -g _sparklebios_ultra_player ""
        end

        function _sparklebios_ultra_sound
            test "$_sparklebios_ultra_sound_on" = 1; or return 0
            test -n "$_sparklebios_ultra_player"; or return 0
            set -l f "$_sparklebios_ultra_sounds/$argv[1].wav"
            test -r "$f"; or return 0
            $_sparklebios_ultra_player $f >/dev/null 2>&1 &
            disown 2>/dev/null
        end

        function _sparklebios_ultra_maybe
            set -l chance $_sparklebios_ultra_chance
            test (count $argv) -gt 0; and set chance $argv[1]
            test (random 0 99) -lt $chance
        end

        function _sparklebios_ultra_everyday
            switch $argv[1]
                case 'git push*' 'ssh *' 'scp *' 'rsync *'
                    _sparklebios_ultra_sound modem
                case 'git pull*' 'git fetch*' 'curl *' 'wget *' 'npm i*' 'pnpm i*' 'cargo build*' 'make*'
                    _sparklebios_ultra_maybe 50; and _sparklebios_ultra_sound modem
                case 'ls' 'ls *' 'la' 'la *' 'll' 'll *' 'tree' 'tree *'
                    _sparklebios_ultra_maybe 10; and _sparklebios_ultra_sound floppy_seek
                case 'git status*' 'git log*' 'git diff*'
                    _sparklebios_ultra_maybe 30; and _sparklebios_ultra_sound floppy_seek
                case 'rm *' 'mv *' 'cp *' 'mkdir *' 'touch *'
                    _sparklebios_ultra_maybe 15; and _sparklebios_ultra_sound hdd_spindown
            end
        end

        function _sparklebios_ultra_rain
            test -t 1; or return 0
            test -n "$NO_COLOR"; and return 0
            set -l secs 1
            test (count $argv) -gt 0; and set secs $argv[1]
            set -l cols $COLUMNS
            set -l rows $LINES
            set -l glyphs '*' '+' '.' 'o' '•' '✦' '✧'
            set -l colours 1 3 2 6 4 5
            set -l dropx
            set -l dropy
            set -l ndrops (math "floor($cols / 3)")
            test $ndrops -lt 1; and set ndrops 1
            printf '\e[?1049h\e[?25l\e[2J'
            for i in (seq 1 $ndrops)
                set -a dropx (random 1 $cols)
                set -a dropy (random 1 $rows)
            end
            set -l start (date +%s)
            while true
                set -l now (date +%s)
                test (math "$now - $start") -lt $secs; or break
                for i in (seq 1 $ndrops)
                    set -l x $dropx[$i]
                    set -l y $dropy[$i]
                    if test $y -gt 1
                        printf '\e[%d;%dH ' (math "$y - 1") $x
                    end
                    set -l c $colours[(random 1 6)]
                    set -l glyph $glyphs[(random 1 7)]
                    printf '\e[%d;%dH\e[9%dm%s\e[0m' $y $x $c $glyph
                    set y (math "$y + 1 + "(random 0 1))
                    if test $y -gt $rows
                        set y 1
                        set x (random 1 $cols)
                    end
                    set dropx[$i] $x
                    set dropy[$i] $y
                end
                sleep 0.04
            end
            printf '\e[2J\e[?25h\e[?1049l'
        end

        function _sparklebios_ultra_chpwd --on-variable PWD
            if test -d .git
                _sparklebios_ultra_maybe 60; and _sparklebios_ultra_sound floppy_seek
            else
                _sparklebios_ultra_maybe 25; and _sparklebios_ultra_sound floppy_seek
            end
            _sparklebios_ultra_maybe; or return 0
            test -n "$NO_COLOR"; and return 0
            set -l entries *
            set -l n (count $entries)
            set -l dir (basename $PWD)
            set -l line ""
            switch (random 0 3)
                case 0
                    set line "Seek complete. $n entries in $dir."
                case 1
                    set -l newest ""
                    set -l newest_mtime 0
                    for f in $entries
                        set -l mtime (stat -f %m -- $f 2>/dev/null; or stat -c %Y -- $f 2>/dev/null)
                        test -n "$mtime"; or continue
                        if test "$mtime" -gt "$newest_mtime"
                            set newest_mtime $mtime
                            set newest $f
                        end
                    end
                    if test -n "$newest"
                        set -l now (date +%s)
                        set -l age (math "floor(($now - $newest_mtime) / 60)")
                        if test $age -lt 60
                            set line "$dir: last write "$age"m ago. Still warm."
                        else if test $age -lt 1440
                            set line "$dir: last write "(math "floor($age / 60)")"h ago."
                        else
                            set line "$dir: last write "(math "floor($age / 1440)")" days ago. Dusty."
                        end
                    end
                case 2
                    if test -d .git
                        set -l branch (git --no-optional-locks symbolic-ref --short HEAD 2>/dev/null)
                        set -l changes (git --no-optional-locks status --porcelain 2>/dev/null | wc -l | string trim)
                        test -n "$branch"; and set line "Mounted $dir on $branch. $changes uncommitted."
                    end
                case 3
                    test $n -eq 0; and set line "$dir is empty. The BIOS respects that."
            end
            test -n "$line"; and printf '\e[90m%s\e[0m\n' "$line"
        end

        function _sparklebios_ultra_precmd --on-event fish_prompt
            set -l ultra_status $status
            if test "$ultra_status" -ne 0; and test "$ultra_status" -ne 130
                set -gx SPARKLEBIOS_STATUS (printf 'ERR 0x%02X' $ultra_status)
            else
                set -gx SPARKLEBIOS_STATUS ""
            end
            test "$_sparklebios_ultra_turbo" = 1; and set -gx SPARKLEBIOS_STATUS "$SPARKLEBIOS_STATUS 66MHz"

            if test -n "$_sparklebios_ultra_cmd"
                set -l now (date +%s)
                set -l ultra_took (math "$now - $_sparklebios_ultra_started")
                if test $ultra_took -ge @@AFTER@@; and test @@PRESENCE@@ = 1
                    if test $ultra_status -eq 0
                        _sparklebios_ultra_sound post_ok
                    else
                        _sparklebios_ultra_sound post_fail
                    end
                else if test $ultra_took -ge 2
                    _sparklebios_ultra_sound hdd_spindown
                end
                if test $ultra_status -eq 0
                    switch $_sparklebios_ultra_cmd
                        case 'git commit*'
                            if test -n "$NO_COLOR"
                                printf '%s\n' 'Saving to CMOS... done.'
                            else
                                printf '\e[36m%s\e[0m\n' 'Saving to CMOS... done.'
                            end
                        case 'git push*'
                            _sparklebios_ultra_rain 1.2
                    end
                    if test "$_sparklebios_ultra_last_failed" = 1; and test $ultra_took -ge 2
                        _sparklebios_ultra_rain 1.6
                    end
                    set _sparklebios_ultra_last_failed 0
                else
                    test "$ultra_status" -ne 130; and set _sparklebios_ultra_last_failed 1
                end
                set _sparklebios_ultra_cmd ""
            end
        end

        function _sparklebios_ultra_preexec --on-event fish_preexec
            set -g _sparklebios_ultra_started (date +%s)
            set -g _sparklebios_ultra_cmd $argv[1]
            _sparklebios_ultra_everyday $argv[1]
        end

        function _sparklebios_ultra_exit --on-event fish_exit
            _sparklebios_ultra_sound power_off
            test -t 1; or return 0
            test -n "$NO_COLOR"; and return 0
            set -l mid (math "floor($LINES / 2)")
            set -l cols $COLUMNS
            printf '\e[?25l\e[2J'
            for w in $cols (math "floor($cols * 2 / 3)") (math "floor($cols / 3)") (math "floor($cols / 8)") 1
                set -l bar (string repeat -n $w '━')
                set -l left (math "floor(($cols - $w) / 2) + 1")
                printf '\e[%d;1H\e[2K\e[%d;%dH\e[97m%s\e[0m' $mid $mid $left "$bar"
                sleep 0.05
            end
            sleep 0.08
            printf '\e[2J\e[H\e[?25h'
        end

        @@JINGLE_LINE@@
    end
"##,
        vars,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> crate::config::Config {
        crate::config::Config::default()
    }

    fn ninja() -> crate::flavour::Flavour {
        crate::flavour::find("ninja", None).unwrap()
    }

    #[test]
    fn a_string_with_a_quote_in_it_survives_being_made_a_shell_word() {
        assert_eq!(Shell::Zsh.quote("plain"), "'plain'");
        assert_eq!(Shell::Zsh.quote("it's"), "'it'\\''s'");
        assert_eq!(Shell::Zsh.quote(""), "''");
    }

    #[test]
    fn fish_escapes_the_backslash_that_ends_a_kitty_escape() {
        // sh and zsh leave a backslash alone inside single quotes. fish does not, so the
        // trailing `ESC \\` of an image would escape the closing quote and swallow the hook.
        assert_eq!(Shell::Fish.quote("a\\b"), "'a\\\\b'");
        assert_eq!(Shell::Zsh.quote("a\\b"), "'a\\b'");
        assert_eq!(Shell::Fish.quote("it's"), "'it\\'s'");
    }

    #[test]
    fn a_flavour_cannot_smuggle_shell_into_the_hook() {
        // A user flavour is a file on disk. Whatever is in it has to come out as text.
        let mut nasty = ninja();
        nasty
            .presence
            .insert("done".into(), "$(touch /tmp/pwned) `id` \"{took}\"".into());
        let out = render(Shell::Zsh, &config(), Some(&nasty), Mascot::none());
        assert!(
            out.contains(r#"'$(touch /tmp/pwned) `id` "'"${took}"'"'"#),
            "the line was not quoted:\n{out}"
        );
    }

    #[test]
    fn a_line_with_no_duration_in_it_is_still_one_quoted_word() {
        assert_eq!(word(Shell::Zsh, "", "\"${took}\""), "''");
        assert_eq!(word(Shell::Zsh, "hi", "\"${took}\""), "'hi'");
        assert_eq!(word(Shell::Zsh, "{took}", "\"${took}\""), "\"${took}\"");
    }

    #[test]
    fn the_flavours_lines_are_written_into_the_hook() {
        let out = render(Shell::Zsh, &config(), Some(&ninja()), Mascot::none());
        assert!(out.contains("'SHINOBI-0'"), "the board name is not in it");
        assert!(out.contains("Ninja: you never saw me."));
        assert!(out.contains("Bad command. If it exists, it is hiding well."));
    }

    #[test]
    fn the_duration_slot_becomes_the_shells_own_variable() {
        let out = render(Shell::Zsh, &config(), Some(&ninja()), Mascot::none());
        assert!(out.contains(r#"'Ninja: done in '"${took}"'. Nobody heard it.'"#));
        // `${took}` contains `{took}`, so counting is the honest check for a leftover slot.
        assert_eq!(
            out.matches("{took}").count(),
            out.matches("${took}").count(),
            "a bare slot was left for nobody to fill"
        );
    }

    #[test]
    fn presence_off_writes_a_hook_that_does_nothing_extra() {
        let off = crate::config::Config {
            presence: false,
            ..config()
        };
        let out = render(Shell::Zsh, &off, Some(&ninja()), Mascot::none());
        assert!(out.contains("[[ 0 == 1 ]]"), "presence is not switched off");
    }

    #[test]
    fn with_no_flavour_at_all_the_hook_is_silent_rather_than_broken() {
        let out = render(Shell::Zsh, &config(), None, Mascot::none());
        assert!(out.contains("[[ 0 == 1 ]]"));
        assert!(!out.contains("{{"));
    }

    #[test]
    fn no_mascot_leaves_the_variable_empty_so_a_prompt_is_unchanged() {
        let out = render(Shell::Zsh, &config(), Some(&ninja()), Mascot::none());
        assert!(out.contains("export SPARKLEBIOS_MASCOT=''"));
    }

    #[test]
    fn a_kitty_placement_sends_the_image_once_and_repeats_only_the_placement() {
        let png = crate::sprite::builtin("ninja").unwrap();
        let mascot = Mascot::kitty(png, 7, 1, Wrap::Zsh);
        assert!(mascot.transmit.contains("a=t"), "the image is not sent");
        assert!(
            !mascot.placement.contains("a=t"),
            "the placement re-sends the whole image"
        );
        assert!(mascot.placement.contains("a=p,i=7,c=2,r=1"));
        assert!(
            mascot.placement.len() < 80,
            "the placement is {} bytes, too many to repeat every prompt",
            mascot.placement.len()
        );
    }

    #[test]
    fn a_streak_worth_mentioning_follows_the_mascot() {
        let png = crate::sprite::builtin("ninja").unwrap();
        let with = Mascot::kitty(png, 7, 1, Wrap::Zsh).with_streak(12);
        assert!(with.placement.ends_with("12d "), "{}", with.placement);
        // One day is not a streak.
        assert_eq!(
            Mascot::kitty(png, 7, 1, Wrap::Zsh).with_streak(1).placement,
            Mascot::kitty(png, 7, 1, Wrap::Zsh).placement
        );
        assert_eq!(
            Mascot::kitty(png, 7, 1, Wrap::Zsh).with_streak(0).placement,
            Mascot::kitty(png, 7, 1, Wrap::Zsh).placement
        );
    }

    #[test]
    fn a_terminal_with_no_images_still_shows_the_streak_on_its_own() {
        // The variable holds the streak and nothing else, so a prompt in Terminal.app gets
        // something out of this too.
        assert_eq!(Mascot::none().with_streak(12).placement, "12d ");
        assert_eq!(Mascot::none().with_streak(1).placement, "");
        assert!(Mascot::none().with_streak(12).transmit.is_empty());
    }

    #[test]
    fn iterm_has_nothing_to_send_once_because_it_stores_nothing() {
        let png = crate::sprite::builtin("ninja").unwrap();
        let mascot = Mascot::iterm(png, 1, Wrap::Zsh);
        assert!(
            mascot.transmit.is_empty(),
            "iTerm2 has no stored image, so there is nothing to transmit up front"
        );
        assert!(mascot.placement.contains("1337;File=inline=1"));
        assert!(
            mascot.placement.len() > 1000,
            "the picture itself is what gets reprinted, so it is not small"
        );
        assert!(mascot.placement.starts_with("%{"), "still width accounted");
    }

    #[test]
    fn a_placement_is_wrapped_so_the_shell_counts_the_right_width() {
        let png = crate::sprite::builtin("ninja").unwrap();
        assert!(Mascot::kitty(png, 7, 1, Wrap::Zsh)
            .placement
            .starts_with("%{"));
        assert!(Mascot::kitty(png, 7, 1, Wrap::Bash)
            .placement
            .starts_with("\\["));
        assert!(Mascot::kitty(png, 7, 1, Wrap::None)
            .placement
            .starts_with('\x1b'));
    }

    #[test]
    fn the_prompt_snippet_expands_the_placement_once_rather_than_needing_prompt_subst() {
        // A single quoted PROMPT needs PROMPT_SUBST, and without it `$SPARKLEBIOS_MASCOT` stays
        // literal and nothing is ever drawn. The snippet we shipped had that bug and no test
        // could see it, because the mistake was in a comment. Found by driving a pty and
        // counting escapes: one transmission, then zero placements.
        assert!(ZSH_HOOK.contains(r#"PROMPT="$SPARKLEBIOS_MASCOT$PROMPT""#));
        assert!(BASH_HOOK.contains(r#"PS1="$SPARKLEBIOS_MASCOT$PS1""#));
    }

    #[test]
    fn fish_replaces_its_own_defaults_and_leaves_the_users_alone() {
        // fish ships a fish_title and a fish_command_not_found, so `functions -q` is true before
        // the hook runs and a plain existence check silently did nothing. Found on a pty: the
        // typo line never appeared in fish, and the title flickered between fish's and ours.
        let out = render(Shell::Fish, &config(), Some(&ninja()), Mascot::none());
        assert!(
            out.contains("_sparklebios_is_fishs_own fish_command_not_found"),
            "the typo handler is back to an existence check, so fish will never install it"
        );
        assert!(
            out.contains("_sparklebios_is_fishs_own fish_title"),
            "the title is back to racing fish's own fish_title"
        );
        assert!(
            !out.contains("not functions -q"),
            "an existence check is not enough in fish"
        );
    }

    #[test]
    fn only_fish_sets_the_title_through_a_function() {
        // zsh and bash have no fish_title, so they print the escape themselves.
        for shell in [Shell::Zsh, Shell::Bash] {
            let out = render(shell, &config(), Some(&ninja()), Mascot::none());
            assert!(out.contains(r"printf '\e]0;%s %s\a' 'SHINOBI-0'"));
        }
    }

    #[test]
    fn every_shells_hook_fills_in_every_placeholder_it_has() {
        for shell in [Shell::Zsh, Shell::Bash, Shell::Fish] {
            let out = render(shell, &config(), Some(&ninja()), Mascot::none());
            assert!(!out.contains("{{"), "a placeholder was left:\n{out}");
        }
    }

    #[test]
    fn zsh_reads_the_precmd_timer_without_tripping_nounset() {
        // Before any preexec has ever run, the very first precmd reads this back: bare
        // `$_sparklebios_started` is "parameter not set" under `setopt nounset`. Found by
        // sourcing the real hook in a pty with nounset on and no preexec fired yet.
        assert!(ZSH_HOOK.contains(r#"[[ -n "${_sparklebios_started:-}" ]]"#));
    }

    #[test]
    fn bash_reads_comp_line_and_the_precmd_timer_without_tripping_set_dash_u() {
        // Same trap in bash, plus COMP_LINE, which the DEBUG trap sees unset outside of
        // completion: bare would be "unbound variable" under `set -u`.
        assert!(BASH_HOOK.contains(r#"[[ -n "${COMP_LINE:-}" ]]"#));
        assert!(BASH_HOOK.contains(r#"[[ -z "${_sparklebios_started:-}" ]]"#));
        assert!(BASH_HOOK.contains(r#"[[ -n "${_sparklebios_started:-}" ]]"#));
    }

    #[test]
    fn bash_only_adds_precmd_and_title_to_prompt_command_once_each() {
        // A second `eval "$(bios init bash)"` in the same shell must not run precmd or the
        // title function twice per prompt: PROMPT_COMMAND is a plain string a second eval
        // would otherwise prepend to again, unguarded.
        assert!(BASH_HOOK.contains(r#"[[ "${PROMPT_COMMAND:-}" != *_sparklebios_precmd* ]]"#));
        assert!(BASH_HOOK.contains(r#"[[ "${PROMPT_COMMAND:-}" != *_sparklebios_title* ]]"#));
    }

    #[test]
    fn a_second_eval_does_not_resend_an_unchanged_mascot() {
        // Each shell keeps the previous placement only long enough to compare against the
        // new one, so a second `eval "$(bios init <shell>)"` (or `source`, for fish) with
        // the same flavour does not resend an image the terminal already has.
        for hook in [ZSH_HOOK, BASH_HOOK, FISH_HOOK] {
            assert!(
                hook.contains("_sparklebios_prev_mascot"),
                "no previous-placement guard:\n{hook}"
            );
        }
    }

    #[test]
    fn mascot_is_gated_on_presence_not_printed_unconditionally() {
        // The regression this guards: the mascot export and image send used to sit outside
        // the presence conditional entirely, so `presence = false` still put a mascot in
        // the prompt. Nothing before the presence gate may mention it, in any shell.
        for hook in [ZSH_HOOK, BASH_HOOK, FISH_HOOK] {
            let presence_idx = hook.find("{{PRESENCE}}").expect("a presence gate");
            assert!(
                !hook[..presence_idx].contains("SPARKLEBIOS_MASCOT"),
                "the mascot is reachable before the presence gate:\n{hook}"
            );
        }
    }

    #[test]
    fn title_is_independent_of_presence_not_nested_inside_it() {
        // The regression this guards: the title used to live inside the same function as
        // the finish line, which is only wired up when presence is on, so `presence =
        // false` silenced the tab title even with `title = true`. `{{NOT_FOUND}}` is the
        // last placeholder every shell's presence block fills in, so it marks that block's
        // far end.
        for hook in [ZSH_HOOK, BASH_HOOK, FISH_HOOK] {
            let presence_idx = hook.find("{{PRESENCE}}").expect("a presence gate");
            let presence_end = hook
                .find("{{NOT_FOUND}}")
                .expect("end of the presence block");
            let title_idx = hook.find("{{TITLE}}").expect("a title gate");
            assert!(
                title_idx < presence_idx || title_idx > presence_end,
                "the title gate is nested inside the presence block:\n{hook}"
            );
        }
    }

    #[test]
    fn the_title_bit_needs_a_flavour_the_same_way_presence_does() {
        // docs/presence.md: title is its own key, independent of presence, but a title
        // still needs a flavour's board name. With no flavour resolved, `{{TITLE_NAME}}`
        // would render as an empty string and the title would become a bare path with a
        // leading space, so `{{TITLE}}` has to fall to 0 exactly when `{{PRESENCE}}` does.
        let flavour = ninja();
        for (presence, title, want_ones) in [
            (true, true, 2),
            (false, true, 1),
            (true, false, 1),
            (false, false, 0),
        ] {
            let cfg = crate::config::Config {
                presence,
                title,
                ..config()
            };
            let out = render(Shell::Zsh, &cfg, Some(&flavour), Mascot::none());
            assert_eq!(
                out.matches("if [[ 1 == 1 ]]").count(),
                want_ones,
                "presence={presence} title={title}:\n{out}"
            );
            assert_eq!(
                out.matches("if [[ 0 == 1 ]]").count(),
                2 - want_ones,
                "presence={presence} title={title}:\n{out}"
            );
        }
        // No flavour at all: both bits fall to 0 regardless of the config keys.
        let cfg = crate::config::Config {
            presence: true,
            title: true,
            ..config()
        };
        let out = render(Shell::Zsh, &cfg, None, Mascot::none());
        assert_eq!(out.matches("if [[ 0 == 1 ]]").count(), 2);
        assert_eq!(out.matches("if [[ 1 == 1 ]]").count(), 0);
    }

    /// The regression the coordinator caught: a hook rendered at anything but `Ultra` must not
    /// mention ULTRA at all, not even in a comment, not even as an inert `eval ''`. This is the
    /// promise that makes the feature safe to ship: a person who has not asked for it gets the
    /// hook they always got.
    #[test]
    fn a_hook_below_ultra_never_mentions_ultra_in_any_shell() {
        for shell in [Shell::Zsh, Shell::Bash, Shell::Fish] {
            for level in [
                crate::sprinkles::Level::Off,
                crate::sprinkles::Level::Light,
                crate::sprinkles::Level::Full,
            ] {
                let cfg = crate::config::Config {
                    sprinkles: level,
                    ..config()
                };
                let out = render_hook(shell, &cfg, Some(&ninja()), Mascot::none(), true, "/sounds");
                assert!(
                    !out.to_lowercase().contains("ultra"),
                    "{shell:?} at {level:?} mentions ultra:\n{out}"
                );
            }
        }
    }

    /// The same guarantee from the other direction: below `Ultra`, `render_hook` is byte for
    /// byte identical to plain `render` (turbo off, no sounds directory), whatever turbo and
    /// sounds_dir it is actually given. ULTRA-only inputs must not leak into a hook that never
    /// reads them.
    #[test]
    fn a_hook_below_ultra_is_unaffected_by_turbo_or_the_sounds_dir() {
        for shell in [Shell::Zsh, Shell::Bash, Shell::Fish] {
            for level in [
                crate::sprinkles::Level::Off,
                crate::sprinkles::Level::Light,
                crate::sprinkles::Level::Full,
            ] {
                let cfg = crate::config::Config {
                    sprinkles: level,
                    ..config()
                };
                let plain = render(shell, &cfg, Some(&ninja()), Mascot::none());
                let with_ultra_state = render_hook(
                    shell,
                    &cfg,
                    Some(&ninja()),
                    Mascot::none(),
                    true,
                    "/anywhere",
                );
                assert_eq!(plain, with_ultra_state, "{shell:?} at {level:?}");
            }
        }
    }

    /// At `Ultra`, the hook carries the flavour's jingle, the configured chance and volume, and
    /// the sounds directory it was given, and none of moments 1 through 9 leak into a hook at
    /// any other level (see the two tests above for that direction).
    #[test]
    fn an_ultra_hook_carries_the_jingle_chance_volume_and_sounds_dir() {
        let cfg = crate::config::Config {
            sprinkles: crate::sprinkles::Level::Ultra,
            ultra_chance: 42,
            ultra_volume: 0.9,
            ..config()
        };
        for shell in [Shell::Zsh, Shell::Bash, Shell::Fish] {
            let out = render_hook(
                shell,
                &cfg,
                Some(&ninja()),
                Mascot::none(),
                true,
                "/tmp/sparklebios-sounds",
            );
            assert!(!out.contains("{{"), "{shell:?} left a placeholder:\n{out}");
            assert!(
                out.contains("jingle_ninja"),
                "{shell:?} has no jingle:\n{out}"
            );
            assert!(out.contains("42"), "{shell:?} has no chance:\n{out}");
            assert!(out.contains("0.9"), "{shell:?} has no volume:\n{out}");
            assert!(
                out.contains("/tmp/sparklebios-sounds"),
                "{shell:?} has no sounds dir:\n{out}"
            );
            assert!(
                out.contains("Saving to CMOS... done."),
                "{shell:?} is missing the ceremony line:\n{out}"
            );
        }
    }

    /// An ULTRA hook with no flavour resolved still renders (silent on the jingle, since there
    /// is nothing to name it after), the same "silent rather than broken" rule presence follows.
    #[test]
    fn an_ultra_hook_with_no_flavour_is_silent_on_the_jingle_but_still_renders() {
        let cfg = crate::config::Config {
            sprinkles: crate::sprinkles::Level::Ultra,
            ..config()
        };
        for shell in [Shell::Zsh, Shell::Bash, Shell::Fish] {
            let out = render_hook(shell, &cfg, None, Mascot::none(), false, "/sounds");
            assert!(!out.contains("{{"), "{shell:?} left a placeholder:\n{out}");
            assert!(
                !out.contains("jingle_"),
                "{shell:?} invented a jingle:\n{out}"
            );
        }
    }

    /// `ultra_volume` at 0 mutes the sound but must not also silence the visual moments (the
    /// folder remarks, the ceremony line, the rain): see `docs/sprinkles.md`'s "Silence" rule.
    #[test]
    fn zero_ultra_volume_mutes_sound_but_keeps_the_ceremony_line() {
        let cfg = crate::config::Config {
            sprinkles: crate::sprinkles::Level::Ultra,
            ultra_volume: 0.0,
            ..config()
        };
        let out = render_hook(
            Shell::Zsh,
            &cfg,
            Some(&ninja()),
            Mascot::none(),
            false,
            "/sounds",
        );
        assert!(out.contains("_sparklebios_ultra_sound_on=0"));
        assert!(out.contains("Saving to CMOS... done."));
    }
}
