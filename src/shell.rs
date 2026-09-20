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
pub fn render(
    shell: Shell,
    config: &crate::config::Config,
    flavour: Option<&crate::flavour::Flavour>,
    mascot: Mascot,
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

    shell
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
        .replace("{{MASCOT_IMAGE}}", &quote(&mascot.transmit))
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
}
