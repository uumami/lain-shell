## ADDED Requirements

### Requirement: Repository docs record untested but adopted integration patterns
The repository SHALL provide a documentation section for integration patterns that are used by the project but have not yet been validated across the target environment matrix.

#### Scenario: Untested but well-used section exists
- **WHEN** `docs/open-questions.md` is reviewed after this change
- **THEN** it contains a dedicated section for “Untested But Well-Used” patterns

### Requirement: Untested integration entries distinguish verified and unverified claims
Each “Untested But Well-Used” entry SHALL state why the pattern is considered acceptable, what has been verified, and what remains unverified.

#### Scenario: Direct X11 icon path is documented honestly
- **WHEN** the direct X11 `_NET_WM_ICON` path is documented in the repository
- **THEN** the entry identifies protocol correctness as the intended outcome and explicitly notes any compositor-specific behavior that remains unverified
