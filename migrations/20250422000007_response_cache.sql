CREATE TABLE response_cache (
    cache_key TEXT PRIMARY KEY NOT NULL,
    virtual_key_id UUID NOT NULL REFERENCES virtual_keys(id),
    model TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    response_body JSONB NOT NULL,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX idx_response_cache_expires ON response_cache (expires_at);
CREATE INDEX idx_response_cache_key_id ON response_cache (virtual_key_id);
