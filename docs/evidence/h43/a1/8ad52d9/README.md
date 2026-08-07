# H43-A1 focused screen evidence

**Candidate:** `8ad52d9600cb7de3c98ee345fd82fb06f9e7d429`<br>
**Candidate tree:** `fdbd8aec276a1519e28f53f1337c689bc73cdbf8`<br>
**Runner:** `6bfe6c94d8d79e0a2a3de86a42c6ad16cfb95108`<br>
**Window:** `2026-08-07T08:02:10Z` through `2026-08-07T08:42:15Z`<br>
**Window status:** closed cleanly<br>
**Machine disposition:** rejected

This directory retains the reviewed summary, frozen plan, analyzer output, and
window state for the H43-A1 forced generic versus forced assertion-specialized
screen. The exact source candidate was benchmark-only and did not change the
supported production matcher or API.

The complete immutable window remains on `agogo.local` at:

```text
/home/rmz/git/h43-a1-screen-20260807T080210Z
```

A compact bundle containing all receipts, host evidence, logs, binaries, and
top-level manifests except the reproducible 541 MiB Cargo target directory is
retained at:

```text
/Users/rmz/.codex/handoffs/rustmatch/h43-a1/h43-a1-screen-20260807T080210Z-compact.tar.gz
```

Its SHA-256 is:

```text
a4a5186479bfb8e9b17972708054cc0ee0b8ca02d391e9421ca17dadd4a3fcdc
```

The remote full-window `sha256.txt` verified every listed file after closure.
The host guard recorded zero contamination lines and zero Docker events. The
three managed services remained exited with restart policy `no`, and the GPU
was idle before and after the window.

## Retained files

| File | SHA-256 |
|---|---|
| `analyzer-console.json` | `75d46a4313ad8ba360231c0f0ba3031abc05cf2f94ec395d4853db27bed73bb0` |
| `frozen-plan.json` | `e4936d302ba8d7a5359a91d9a3ab1661c6a5bf58574d1af720d6b3bbd1cbdd90` |
| `summary.json` | `e9ec32f1ca7f730bec48d0b8b135dcfaa96a0fd85cfff40b5540a5c961a5a2fd` |
| `window-state.json` | `412abf9f5f368ba85adf6798fcea34d5fe7ea55139d6df8b4bd8cb9c450e71ae` |

See the [reviewed result](../../../../experiments/h43-a1-result.md) for the
machine ruling, causal interpretation, and next bounded experiment.
