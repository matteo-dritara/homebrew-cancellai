# Guardian Model

Guardian is a local observer and bounded remediation client of the same cancellAI engine. It is not a second safety implementation.

## Three separated concerns

### Detection

What is happening?

- free-disk capacity;
- provider/project budgets;
- growth velocity and acceleration;
- baseline deviation;
- session-count explosion;
- unexpected giant artifacts;
- provider layout drift;
- orphan-state growth.

### Decision

What would improve the situation?

The engine constructs candidate recommendations/plans using the normal artifact and policy model.

### Authority

What may cancellAI actually do?

Only the Effective Authority permits actions. Pressure or anomaly severity never self-escalates authority.

## Pressure states

The exact function is calibrated later, but the semantic states are:

- `GREEN` - normal.
- `YELLOW` - approaching soft budget/pressure; surface context.
- `ORANGE` - material risk; recommend or execute pre-authorized reversible actions.
- `RED` - critical disk/budget trajectory; prioritize safe remediation but do not bypass constitutional limits.

Hysteresis prevents notification/action flapping.

## Forecasting

Forecasts can include estimated time to disk pressure or budget exhaustion. They must surface insufficient-data/uncertainty states and are never authorization inputs by themselves.

## Baselines

Baselines are local and metadata-only. The first implementation should prefer transparent robust statistics/heuristics over opaque ML. More sophisticated models are allowed only if their output remains advisory evidence and can be explained sufficiently for debugging.

## Runtime

Target user-service mechanisms:

- macOS: `launchd` user agent.
- Linux: `systemd --user` where available, explicit fallback otherwise.
- Windows: user-scoped scheduled task/service design chosen through platform ADR.
- WSL: separate environment behavior; do not silently install a Windows host service from the Linux guest.

## Kill switch

Guardian must have an immediate local disable path. Disabling automation never prevents manual read-only inspection or recovery. Any in-flight destructive action still follows safety executor transaction semantics rather than being killed mid-syscall unsafely.

## Audit

Every Guardian remediation/recommendation records:

- detection evidence;
- pressure/anomaly state;
- policy resolution;
- authority result;
- sealed plan ID if any;
- execution result.
