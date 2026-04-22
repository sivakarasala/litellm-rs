CREATE TABLE approved_emails (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email TEXT NOT NULL UNIQUE,
    display_name TEXT,
    provider_key_id UUID REFERENCES provider_keys(id),
    allowed_models TEXT[],
    max_budget_usd NUMERIC(10,4),
    budget_reset_period TEXT CHECK (budget_reset_period IN ('daily', 'monthly') OR budget_reset_period IS NULL),
    rpm_limit INTEGER,
    tpm_limit INTEGER,
    default_expiry_hours INTEGER DEFAULT 720,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_approved_emails_email ON approved_emails (email);
