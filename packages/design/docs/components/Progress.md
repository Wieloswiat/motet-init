# Progress

How far a task has got, or that one is running.

- `Progress({ label, value, max, valueText })`; without a value it is indeterminate. `ProgressRing({ value, size, showValue, label })`; `Spinner({ size, label })`.
- Bars and rings are `role=progressbar` with their values; spinners always have a label.
- Under reduced motion, indeterminate bars stop and spinners stand still; their words carry the state.
