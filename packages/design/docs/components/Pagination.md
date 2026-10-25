# Pagination

Pages of a long Scene or list.

- `Pagination({ page, count, href })`. With `href(n)` the pages are links; without it, buttons that emit `change`.
- Shows the first and last page, the current page with its neighbours, and an ellipsis for the gaps. The current page has `aria-current="page"`.
- Below 420px of container width only the arrows and "7 / 12" remain.
