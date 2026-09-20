# Columns and marks

What every column in the process table means, and what every mark in it
means. Which columns are drawn depends on the view (`v`), on what the
platform publishes, and on how wide the terminal is — see
[views](views.md) and [platforms](platforms.md).

## Columns

| Header | What it is |
|---|---|
| `CPU%` | Share of one core over the last interval. A threaded process can exceed 100%: four cores busy is `400`. |
| (bar) | The same figure as a bar, so a column of numbers can be scanned. `+` marks a process past one core. |
| `RSS` | Resident memory: pages actually in RAM. Shared pages are counted in full for every process sharing them. |
| (bar) | RSS as a share of the machine's memory. |
| `S` | Process state: `R` running, `S` sleeping, `I` idle, `T` stopped, `Z` zombie, `X` exited during this interval, `?` not readable. |
| `THR` | Threads. `—` where the platform will not say. |
| `DISK R`, `DISK W` | Bytes a second to and from the device, not the page cache. |
| `HIST ≤N%` | The process's own CPU history, oldest left. The scale is shared by every row so their shapes can be compared, and the header states its ceiling. Drawn only when some process's history moved; when none did, the title says `history flat`. |
| `PID` | The process id — or `×N` on a folded row, which stands for N processes rather than one. |
| `USER` | The owner. Folded into the panel title when every process has the same one. |
| `CID` | Container id, twelve characters as `docker ps` shows it. Only on a machine running containers. |
| `COMMAND` | The command line, or the name where there is none. Elided in the middle, so both ends survive. |

The memory view (`v`) replaces the middle columns with:

| Header | What it is |
|---|---|
| `PSS` | Proportional set size: shared pages divided among the processes sharing them, so a pool of workers sums to what it really costs. Linux only. |
| `VSZ` | Virtual size: address space mapped. On macOS this includes the shared region every process maps, so hundreds of gigabytes is normal and says nothing. |
| `MAJF/s` | Major faults a second: pages fetched from disk. This is the column that answers "why is this slow while its CPU looks fine". |
| `GROW` | Change in RSS since the previous sample, signed. `·` for no change. |

## Marks

| Mark | Meaning |
|---|---|
| `—` | Not known. The platform does not publish it, or this user may not read it. Never a zero. |
| `·` | A real nothing: zero movement, or a figure from before the column was switched on. |
| `?` | For a state: the kernel would not say. For a user: the owner is unknown. |
| `×24` | A folded row standing for 24 processes. Its CPU, memory and threads are summed; its state, user, history and command line are `—` or blank, because a group has no single one of them. |
| `█ ▊ ▏` | Bar fill, in eighths of a cell. |
| `████+` | A bar at full scale with more beyond it: a process past one core. |
| `⣀⣤⣶` | Braille history. Each cell is two samples wide and four levels tall; blank means the process was not running then. |
| `+352.0K`, `-2.1M` | Growth since the previous sample, in the memory view. |

## Why a column disappeared

poptop drops a column rather than show the same thing in every row, and
says so in the panel title when it does.

| It says | Why |
|---|---|
| `· all oddurs` | Every process has one owner, so `USER` would be that word repeated. The title carries the fact instead. |
| `· all oddurs but 201 unknown` | The same, where some processes' owners could not be read. |
| `· history flat` | No process's history moved, so the sparkline would be the CPU column drawn again. |
| `! io: panel too narrow` | The disk columns need width the terminal does not have. |
| `! io: 196/781 need root` | `/proc/<pid>/io` is readable only for your own processes. |
| `· 100 git (g folds them)` | Not a dropped column: an offer. One program is crowding the table and `g` folds it. |

On a terminal too narrow for everything, columns go in this order: the
history, the bars, the container, the user, the thread count, the view's own
columns, the state, the memory, and last the pid. `CPU%` and the command
always stay, and a number is never cut short to make something fit.
