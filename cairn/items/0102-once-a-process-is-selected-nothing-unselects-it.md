---
id: 102
title: Once a process is selected, nothing unselects it
type: bug
status: done
milestone: later
labels:
- review
created: 2026-09-18
updated: 2026-09-19
priority: p3
effort: s
area: app
---

## What happens

`App::selected` is set by the arrow keys and by nothing else, so no key sets it back to `None`. Once a process has been picked, poptop follows it for the rest of the session. `watched_but_absent` keeps saying it is not there after it exits, and `d` shows its history, not the machine's. The only way back to "nothing selected" is to quit.

Found in the 0089 review of `main.rs` and `app.rs`, as the one state transition no key undoes.

## What should happen

A key that clears the selection. The natural one is Esc, but in the main table Esc currently quits, and changing that is a decision about the key map, not a bug fix. Other choices are a second press of the arrow at the edge of the list, or `u`.

## Reproduction

1. Press Down to select a process.
2. Try to return to no selection.

## How it was resolved

**Esc backs out one level, and a selection is now one of the levels.** Esc already worked that way in the filter box, the jump box and the signal prompt: leave what's open, and quit only when nothing is. The selection was the one mode without a way out, so it joins the list rather than getting a key of its own. With a process selected, the first Esc lets go of it and the second quits. `q` still quits at once, so nobody who quits with `q` notices a difference. Someone who quit with Esc while a process was selected now presses it twice, and the first press visibly does something.

`App::deselect` clears the per-process history view (`d`) along with the selection. That view is *of* the selection, and left on it would show the machine's timeline captioned "pick a process first". Thread expansion (`y`) is kept: it's a way of looking at whichever process gets selected next.

Tested in `a_selection_is_a_mode_and_esc_leaves_it_before_it_quits`. The `--help` text and the README's key table say what Esc does.
