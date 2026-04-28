<!-- 
SPDX-FileCopyrightText: 2026 Knitli Inc 

SPDX-License-Identifier: MIT OR Apache-2.0
-->
<!--   
Jules:  Note that the current year is 2026. That is not an error. You should check the actual current date before recording a date in this log. Remember that your training was over a year ago so 2026 might 'feel' like the future, but it is the present.
-->
# Bolt Journal

## 2026-04-23 - BTreeSet Bulk Insertion
**Learning:** Manual nested loops using `.insert()` on `BTreeSet` for nested structures (like compartments and sub-compartments) prevents bulk allocation optimizations and increases redundant traversals.
**Action:** Use `.extend()` combined with iterator chains (`.map()` or `.cloned()`) when populating sets or collections from nested structures to leverage iterator optimizations and bulk insertions.

## 2026-04-23 - SPDX License Headers
**Learning** New files in this repository require SPDX license headers. Documentation and config files are `MIT OR Apache-2.0` while source code are `LicenseRef-MarqueLicense-1.0`.
**Action:** When creating a new file, ensure it has license and copyright headers in the SPDX format.

### Reference Lifetimes Over Owned Data
* When returning collections derived from struct fields or method arguments, prefer yielding collections of lifetimes-bound references (e.g., `Vec<(&'a str, &'a str)>`) instead of eagerly allocating owned types (`Vec<(String, String)>`) to avoid unnecessary `.to_owned()` and `.clone()` heap allocations.

## 2026-04-28 - Conditional Map Insertion Optimization
**Learning:** Using `.entry(k.to_string()).or_default()` in a hot loop unconditionally allocates a string for the lookup key even if the entry already exists, which bypasses the zero-allocation fast path for map hits. However, blindly converting all `entry()` usages to conditional `contains_key()` checks is an anti-pattern when the lookup key is *already an owned value* unconditionally created outside the map check, because this causes a redundant clone.
**Action:** Replace `map.entry(k.to_string()).or_default()` with `if !map.contains_key(k) { map.insert(k.to_string(), ...); } map.get_mut(k).unwrap();` where `k` is a borrowed reference (`&str`), but leave `map.entry(k).or_default()` alone if `k` is an already-owned value.
