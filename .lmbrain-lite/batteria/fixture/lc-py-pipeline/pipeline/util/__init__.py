"""Small, stateless helpers shared across the packages above.

Nothing in `util` imports from `readers`, `validators`, `stages`,
`aggregators`, or `writers`: dependencies only ever point inward, into
`util`, never back out of it.
"""
