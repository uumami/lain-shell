## MODIFIED Requirements

### Requirement: Navi output broadcast reflects Core-owned VTE
The multi-client output broadcast section in `quanta/navi/systems-architecture.md` SHALL state that Core updates the cell grid and emits cell diffs. Navi SHALL route diffs to clients based on view state. Navi SHALL NOT parse VTE or update cell grids.

#### Scenario: Output broadcast description
- **WHEN** reading the multi-client output broadcast section
- **THEN** it says Core feeds PTY bytes into Term, emits cell diffs, and Navi routes diffs to clients — not "VTE parser updates the shared cell grid"
