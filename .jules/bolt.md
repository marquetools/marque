# Bolt Journal

## 2024-05-23 - BTreeSet Bulk Insertion
**Learning:** Manual nested loops using `.insert()` on `BTreeSet` for nested structures (like compartments and sub-compartments) prevents bulk allocation optimizations and increases redundant traversals.
**Action:** Use `.extend()` combined with iterator chains (`.map()` or `.cloned()`) when populating sets or collections from nested structures to leverage iterator optimizations and bulk insertions.
