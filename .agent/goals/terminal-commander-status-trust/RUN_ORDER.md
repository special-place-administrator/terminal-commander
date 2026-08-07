# Run Order - terminal-commander-status-trust

```text
STE01  Outcome Evidence Foundation          (blocks everything)
  |
  +-- STE02  Lane Ownership For Status Reads   (live defect; blocks STE04)
  |     |
  |     +-- STE04  Provenance Taxonomy And Agent Contract
  |           |
  +-- STE03  Reconstructed Outcome Evidence    (reported loss)
  |     |     |
  |     |     +-- STE07  Delivery Mode Parity
  |     |
  +-- STE05  Lost Detection
        |
        +-- STE06  Abandonment Records  (needs STE04 for the trust value)
              |
              +-- STE08  Contract Sweep And Verification Gate
```

MVP is STE01 + STE02 + STE03. STE02 precedes STE04 because an `observed` trust
value on a cross-lane read would certify a falsehood.

STE02 and STE03 are independent of each other and may run concurrently once
STE01 lands.
