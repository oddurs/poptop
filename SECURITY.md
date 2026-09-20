# Security

## What poptop does to your machine

It reads. By default poptop opens files the kernel publishes — `/proc` on
Linux, the `sysctl` and `proc_pidinfo` interfaces on macOS — and writes nothing
outside its own state directory. It opens no sockets, talks to no network, and
starts no daemon.

Two things change that, and both are off unless you turn them on:

- `--signals=on` lets `x` and `X` send `TERM` and `KILL` to the process you have
  selected, after a confirmation that names it. See
  [docs/guide/signals.md](docs/guide/signals.md) for the rules poptop holds
  itself to — including that it never signals while you are scrubbing history,
  and rechecks the process's identity with the kernel as it sends.
- `--store=on` and `--log=on` write history under `$XDG_STATE_HOME/poptop`
  (`~/.local/state/poptop`). Those files hold process names, command lines and
  usernames from your machine.

Running poptop as root shows you other users' processes and their command
lines. It does not need root for its own sake.

## Reporting something

If you have found a way for poptop to damage a machine, expose something it
should not, or act on a process it was not asked to act on, report it privately
through GitHub's **Report a vulnerability** on the Security tab, or by email to
the address on the maintainer's GitHub profile. Please do not open a public
issue first.

Tell us what you did, what happened, and what you expected. A proof of concept
is welcome and not required.

You will get an acknowledgement. A fix, or a decision that it is not a
vulnerability with the reasoning, follows on the repository where everyone can
read it.

## What counts

Anything that makes poptop act on the machine beyond reading it, or report a
figure in a way that could be trusted and is not true. Crashes matter: a monitor
that dies on a malformed file is a monitor that is not there when you need it,
which is why the readers are fuzzed (see [docs/design/](docs/design)).

Anything that requires an attacker to already be able to write your
configuration, your theme files or your state directory is not a vulnerability —
at that point they are you.
