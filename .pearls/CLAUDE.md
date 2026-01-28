# Pearl Issue Tracking

This project uses `pearl` for issue tracking - a daemon-less, file-based issue tracker.

## Before starting work
```bash
pearl ready           # See what's ready to work on
pearl list -s open    # List all open issues
```

## When starting a task
```bash
pearl update <ID> -s in_progress
```

## When completing a task
```bash
pearl close <ID>
```

## Creating new issues for bugs/features discovered
```bash
pearl create "Title" -t bug -p 1      # Bug, high priority
pearl create "Title" -t feature -p 2  # Feature, medium priority
pearl create "Title" -t task          # Task (default)
```

## Checking project status
```bash
pearl stats
```

## Guidelines

- Check `pearl ready` at the start of a session to see available work
- Create issues for any bugs or improvements discovered during development
- Update issue status when starting/completing work
- Use appropriate priorities: P0=critical, P1=high, P2=medium, P3=low, P4=backlog
