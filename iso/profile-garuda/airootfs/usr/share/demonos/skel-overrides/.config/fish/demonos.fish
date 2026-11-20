# DemonOS PhantomCore additions to Garuda's fish config (sourced from
# ~/.config/fish/config.fish right after Garuda's defaults; added by demonos-rebrand).
if status is-interactive
    set -g fish_greeting "DemonOS PhantomCore // Lillith is listening locally. (red-team = local simulation only)"
end
# Garuda calls __garuda_fastfetch at the end of config.fish; show the DemonOS preset instead.
function __garuda_fastfetch
    if status --is-interactive && type -q fastfetch
        fastfetch --config /usr/share/fastfetch/presets/demonos.jsonc
    end
end
