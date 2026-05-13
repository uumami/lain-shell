## ADDED Requirements

### Requirement: Stable prompt identity marker
The lain-shell prompt SHALL render the `lambda_` identity marker with the same foreground color regardless of the previous command exit status.

#### Scenario: Success keeps identity color
- **WHEN** the previous command exits with status 0
- **THEN** the prompt renders `lambda_` in the configured identity color

#### Scenario: Failure keeps identity color
- **WHEN** the previous command exits with a non-zero status
- **THEN** the prompt renders `lambda_` in the same configured identity color

### Requirement: Failure status is separate from identity marker
The lain-shell prompt SHALL indicate non-zero exit status with a compact marker separate from the `lambda_` identity marker.

#### Scenario: Failed command shows status marker
- **WHEN** the previous command exits with a non-zero status
- **THEN** the next prompt includes a visible `!` status marker before the prompt marker

#### Scenario: Successful command hides status marker
- **WHEN** the previous command exits with status 0
- **THEN** the next prompt does not include the failure status marker

### Requirement: Prompt layout remains compact
The lain-shell prompt SHALL preserve the existing left-side layout of current directory, optional git branch, status marker, colon, and `lambda_` marker.

#### Scenario: Git repository prompt
- **WHEN** the shell is inside a git repository on branch `main`
- **THEN** the prompt renders the current directory, `(main)`, optional failure marker, `:`, and `lambda_` on the same line

#### Scenario: Non-git directory prompt
- **WHEN** the shell is outside a git repository
- **THEN** the prompt renders the current directory, optional failure marker, `:`, and `lambda_` without an empty branch placeholder
