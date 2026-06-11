## ADDED Requirements

### Requirement: One Self-Contained HTML Page For All Benches

The suite SHALL render all benchmark output — criterion trend history and balance-study reports — on a **single self-contained HTML page** (inline data, inline styles/scripts) that renders fully when opened from disk with no network access and makes zero external requests.

#### Scenario: Offline render

- **WHEN** the generated page is opened from the filesystem with networking disabled
- **THEN** every chart and report section renders, and no external request is attempted

### Requirement: The Dashboard Is Regenerated Per Main Merge

On every merge to `main`, after the bench job appends its history record, the dashboard SHALL be regenerated from the full `bench-data` history and published (as a CI artifact; via the delivery pipeline's static hosting once `boom2-delivery` lands). Criterion trends SHALL be drawn with confidence/noise bands so the 6–12% rerun variance is visible against any real shift.

#### Scenario: A merge refreshes the page

- **WHEN** a commit merges to `main`
- **THEN** the published dashboard includes that commit's bench record at the end of every trend line

#### Scenario: Noise is visually distinguishable

- **WHEN** a reader inspects a bench trend
- **THEN** confidence/noise bands make single-point wobble distinguishable from a sustained level shift

### Requirement: On-Demand Study Reports Fold Into The Same Page

Regenerating the dashboard SHALL pick up any balance-study reports present in the history, so one page stays the single reading surface for both instruments — locally during a study session as well as in CI.

#### Scenario: A local study appears on the local page

- **WHEN** an on-demand balance study completes and the dashboard is regenerated locally
- **THEN** the new study's report (metrics, matrix cells, provenance) appears on the page alongside the criterion trends
