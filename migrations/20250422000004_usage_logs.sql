CREATE TABLE usage_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    virtual_key_id UUID NOT NULL REFERENCES virtual_keys(id),
    provider_key_id UUID NOT NULL REFERENCES provider_keys(id),
    model TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    cost_usd NUMERIC(10,6) NOT NULL DEFAULT 0,
    cached BOOLEAN NOT NULL DEFAULT false,
    status_code INTEGER NOT NULL,
    latency_ms INTEGER NOT NULL DEFAULT 0,
    request_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_usage_logs_virtual_key ON usage_logs (virtual_key_id, created_at DESC);
CREATE INDEX idx_usage_logs_created_at ON usage_logs (created_at DESC);
