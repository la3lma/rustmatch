# H43-A1-P1 preparation discriminator evidence

This package retains the reviewed machine output for the clean H43-A1-P1
phase-local AB/BA construction window.

## Identity

- Candidate: `8ad52d9600cb7de3c98ee345fd82fb06f9e7d429`
- Candidate tree: `fdbd8aec276a1519e28f53f1337c689bc73cdbf8`
- Runner: `f5cabeef7129fff6367671ff2d8c6baf1f1cced9`
- Plan SHA-256:
  `02903ae31638d4e3da220530224f6aef1d7a11d6c3b8eb4eefd2206e9e692753`
- Window: `2026-08-07T09:35:59Z` through `09:36:09Z`
- Status: `closed-cleanly`

## Included files

- `frozen-plan.json`: predeclared fixtures, source hashes, ordering, thresholds,
  and decisions.
- `summary.json`: retained structured analyzer result.
- `analyzer-console.json`: exact analyzer standard output.
- `window-state.json`: clean terminal state bound to the plan and candidate.

Raw per-build receipts, host snapshots, test logs, and the complete SHA-256
manifest remain in the full remote evidence directory:

```text
/home/rmz/git/h43-a1-p1-screen-20260807T093559Z
```

The compact external bundle includes those receipts and also preserves the
failed pre-measurement `09:35:05Z` launch sidecars:

```text
/Users/rmz/.codex/handoffs/rustmatch/h43-a1-p1/h43-a1-p1-screen-20260807T093559Z-compact.tar.gz
```

Its SHA-256 is:

```text
cd014709818bc76c422f7656494ad7f201ff03af91b15f39aa54ec5f21ea9ab0
```

See the [reviewed result](../../../../experiments/h43-a1-p1-result.md) for the
causal interpretation and authorize/close decision.
