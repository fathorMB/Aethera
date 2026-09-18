# pipeline

A small batch pipeline that turns raw order and customer feeds into a sales
report: read, validate, normalize, dedupe, convert currencies, aggregate,
rank, write.

## Shape

```
pipeline/
  domain/       plain data records (Customer, Product, Order, Refund)
  readers/      turn a CSV or JSONL file into domain records
  validators/   per-record rules; a record either passes whole or is rejected
  stages/       record -> record transforms that run over a whole batch
  aggregators/  batch -> summary (totals, grouping, ranking)
  writers/      summary -> text (CSV, JSON, Markdown, fixed-width)
  reports/      higher-level report builders on top of stages+aggregators
  plugins/      an opt-in registry for extra stages
  util/         small stateless helpers shared across the packages above
  orchestrator.py   wires the stages above into `run(orders, customers, config)`
  cli.py            command-line entry point
```

## Pipeline order

`orchestrator.run` does, in this order:

1. normalize customers and orders (trim strings, uppercase currency/country codes)
2. validate each order; rejects are recorded but do not stop the run
3. dedupe orders by `order_id`, keeping the first occurrence
4. filter by date range, if the config asks for one
5. sort the surviving orders by date
6. convert each order's total into the report's target currency
7. group the converted totals by customer
8. rank customers by total spend, highest first

The rule for step 8, spelled out in `pipeline/util/ordering.py`: when two or
more customers have the exact same total, the ranking must break the tie by
the customer's name, ascending, so that the same input always produces the
same report no matter what order the orders happened to arrive in.

## Running the tests

```
python -m unittest discover -s tests -t .
```
