# GraphCraft asset inventory

Every binary asset in this repository (image, icon, font, cursor, example
dataset, etc.) must be listed here with its provenance and licence. See
[`AGENTS.md`](AGENTS.md) › Assets.

`cargo xtask assets` fails if the inventory is missing a tracked asset or
lists one that no longer exists.

| Path | Author | Source | Licence | Notes |
| --- | --- | --- | --- | --- |
| _(none yet)_ | | | | |

## Public-domain datasets allowed as test fixtures

Numerical datasets published in the public domain are **not** binary assets
and are allowed under `tests/data/` with their source URL recorded in the
dataset's companion `*.source.txt` file. Specifically:

- NIST Statistical Reference Datasets (StRD) — public domain, U.S. federal
  work: <https://itl.nist.gov/div898/strd/>
