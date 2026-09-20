# Prior art, and where poptop actually differs

poptop is not the first tool to let you look backwards, and it is not the most
capable one.

**[atop](https://www.atoptool.nl/)** has recorded historical per-process data
for years, and it remains the tool to install if you are running a fleet. It
records **by default**, where poptop records nothing until you ask; it keeps 28
days once it does, where poptop's log keeps seven; `atopsar` prints twenty-four
kinds of report where poptop prints four; it reaches GPUs and per-process
network through daemons poptop declines to require, and Infiniband natively,
which poptop simply has not done; and it is a system service, which is a
different thing from a program you run.

Several things it used to do alone, poptop now does too — capturing processes
that lived and died between two samples, surviving a restart, jumping to a
timestamp, summarising a period, machine-readable output. The tables below set
that out, and they carry the rows poptop loses.

**[zenith](https://github.com/bvaisvil/zenith)** has zoomable scroll-back charts
and saves data between runs. Its scrollback is aggregate-only, though: its
history holds CPU, memory, network, disk and GPU series, and its process table
renders from a live map that drops pids as they exit, so scrolling back moves
the charts but not the table.

**htop, btop and bottom** keep no history at all. They render the current
instant.

What poptop offers is one claim, and it is worth stating as narrowly as it is
true. It is not that nobody else does this:

- **Nothing has to have been running.** atop can only replay what its daemon
  already recorded. The common case — you connect to a machine that is slow
  *now* — is the case where that daemon was not running. poptop gives you the last
  ten minutes from a cold start.
- **One view, live and historical.** Scrubbing happens inside the running
  monitor, not in a separate replay mode against a logfile.
- **The process table follows the cursor.** Scrub to a spike and the table below
  it is the one from that instant.

When this was first written the honest summary was "a usability position, not a
capability one — atop does more". The first half still holds and the second no
longer does, which is a change that invites overstating. So, precisely: on the
box you have just connected to, poptop answers questions atop cannot, because
atop was not running. On a fleet you administer, atop answers questions poptop
cannot, because it *was*. Install atop if you want history you can rely on after
the fact across many machines. Use poptop when you want to know what this box is
doing now and what it was doing a few minutes ago — including on the several
thousand machines where nobody installed anything in advance.

## Where each of them stands on time

|                                                | poptop | htop | btop | bottom | zenith | atop |
| ---------------------------------------------- | :--: | :--: | :--: | :----: | :----: | :--: |
| Live view                                      |  ●   |  ●   |  ●   |   ●    |   ●    |  ●   |
| Rolling graph of recent values                 |  ●   |  ◐¹  |  ●   |   ●    |   ●    |  ○   |
| Move backwards through time                    |  ●   |  ○   |  ○   |   ◐²   |   ●    |  ●³  |
| Process table follows the time cursor          |  ●   |  ○   |  ○   |   ○    |   ○⁴   |  ●   |
| Captures processes that exited between samples |  ●⁵  |  ○   |  ○   |   ○    |   ○    |  ●   |
| History survives a restart                     |  ●⁶  |  ○   |  ○   |   ○    |   ●    |  ●   |
| **Needs something running beforehand**         |  ○   |  ○   |  ○   |   ○    |   ○    |  ●⁷  |

● yes · ◐ partial · ○ no

1. htop's Graph meter mode (`GRAPH_METERMODE`, `Meter.c`) keeps a rolling
   scalar buffer sized to the meter width. It is a graph, not navigable
   history, and it is per-meter rather than per-process.
2. bottom can freeze the display (`f`), but freezing gates the update
   (`if !app.data_store.is_frozen()`, `lib.rs`) rather than letting you look
   backwards. poptop keeps sampling while you scrub.
3. atop steps through intervals when replaying a logfile (`atop -r`), which is
   a separate mode rather than the live view — although `atop -t` ("twin mode:
   live measurement with possibility to review earlier samples") narrows that.
4. zenith's `HistogramKind` holds only aggregate series; its process table
   renders from a live map that runs
   `.retain(|&k, _| current_pids.contains(&k))`.
5. Over `taskstats` exit records, which need `CAP_NET_ADMIN`, the initial PID
   namespace **and** the initial network namespace — the third being the one
   that fails silently, since registration succeeds and the records then go
   nowhere. Where any of them is missing the panel says so.
6. Two ways, both off by default: a buffer written on a clean exit (`store`),
   and a daily log appended while running (`log`).
7. atop's history requires its daemon to have been recording in advance. **This
   row is the whole of poptop's argument**, and it is the only one that has not
   moved.

Two cells changed in this table when v2.0–v2.2 landed, both in poptop's column.
Everything else that changed is in the table below instead, against atop alone:
putting those eight rows here would have meant thirty-two cells for htop, btop,
bottom and zenith that nobody had checked, which is a worse table rather than a
fuller one.

## Where poptop and atop differ now

Both columns verified together, so this is the comparison with the fewest
unstated assumptions in it.

|                                              | poptop | atop |
| -------------------------------------------- | :--: | :--: |
| Jump to a timestamp                          |  ●⁸  |  ●⁹  |
| Summarise a period without watching it       |  ●   |  ●¹⁰ |
| Machine-readable output                      |  ●¹¹ |  ●¹² |
| **Keeps history without being asked**        |  ○¹³ |  ●   |
| **Weeks of retention**                       |  ○¹⁴ |  ●¹⁵ |
| **GPU, per-process network**                 |  ○   |  ◐¹⁶ |
| **Infiniband**                               |  ○¹⁷ |  ●   |
| **Renice a process**                         |  ○   |  ●   |

8. `b`, taking both an absolute time and a relative one, and saying so when
   nothing was recorded at the moment asked for rather than showing the nearest
   sample as though it were.
9. `atop -r file -b [YYYYMMDD]hhmm[ss]`.
10. `atopsar`, whose report list is far longer than poptop's — twenty-four
    report types against poptop's four, including per-protocol IP, ICMP, UDP
    and TCP for both address families.
11. `--export=json|line`, plus `--schema`, which atop has no equivalent of: its
    label set is documented in a man page rather than emitted.
12. `atop -P label[,label]` and `-J` for JSON.
13. The row that is the point. atop is a system service and records by default;
    poptop records nothing at all until `store` or `log` is turned on, and gives
    you the last ten minutes either way.
14. Seven days and 512 MB **once `log` is on**. poptop keeps whole process
    tables in every entry, so the same retention as atop's would cost far more;
    the measured figures are in **Opening yesterday**.
15. `LOGGENERATIONS=28`, `LOGINTERVAL=600`, `LOGPATH=/var/log/atop`.
16. `atop -k` connects to an external `atopgpud` daemon for GPU, and `-K` to
    `netatop`/`netatop-bpf` for per-process network — separate things to install
    and run, which is the line poptop draws in **What poptop will read, and what
    it will not**.
17. Not a daemon question: `/sys/class/infiniband` publishes port counters to an
    ordinary reader, so this would qualify under poptop's own rule. It is out on
    scope, and the machines that have it have fabric monitoring already.

Every cell in the first table was checked against the tool's source or official
documentation rather than from memory; the footnotes name where. **atop's
column in both tables was verified on 2026-09-09 against atop 2.11.1** —
`atop -h`, `atopsar`'s report list and `/etc/default/atop` on a Debian install.
poptop's cells were checked against this build. The htop, btop, bottom and
zenith cells are the ones from the original table, checked when it was written
and not re-run since; no cell has been added to those columns without a check.
