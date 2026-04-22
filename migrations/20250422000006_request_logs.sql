CREATE TABLE request_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    usage_log_id UUID NOT NULL REFERENCES usage_logs(id),
    virtual_key_id UUID NOT NULL REFERENCES virtual_keys(id),
    request_body JSONB NOT NULL,
    response_body JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_request_logs_virtual_key ON request_logs (virtual_key_id, created_at DESC);
