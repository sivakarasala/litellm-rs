CREATE TABLE budget_counters (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    virtual_key_id UUID NOT NULL REFERENCES virtual_keys(id),
    period_start TIMESTAMPTZ NOT NULL,
    period_end TIMESTAMPTZ NOT NULL,
    total_cost_usd NUMERIC(10,4) NOT NULL DEFAULT 0,
    total_tokens BIGINT NOT NULL DEFAULT 0,
    request_count INTEGER NOT NULL DEFAULT 0,
    UNIQUE (virtual_key_id, period_start)
);
