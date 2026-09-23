CREATE TABLE pull_requests (
    id TEXT PRIMARY KEY NOT NULL,
    number INTEGER NOT NULL,
    title TEXT NOT NULL,
    state TEXT NOT NULL,
    url TEXT NOT NULL,
    repository TEXT NOT NULL,
    author TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    ci TEXT NOT NULL,
    approvals INTEGER NOT NULL,
    required_approvals INTEGER NOT NULL,
    has_conflicts INTEGER NOT NULL
);

CREATE TABLE sync_states (
    id TEXT PRIMARY KEY NOT NULL,
    watermark TEXT NOT NULL
);
